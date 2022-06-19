use glad_gl::gl;
use std::collections::HashMap;

#[derive(Default)]
struct UserTexture {
    size: (usize, usize),

    /// Pending upload (will be emptied later).
    pixels: Vec<u8>,

    /// Lazily uploaded
    texture: Option<gl::GLuint>,

    /// For user textures there is a choice between
    /// Linear (default) and Nearest.
    filtering: bool,

    /// User textures can be modified and this flag
    /// is used to indicate if pixel data for the
    /// texture has been updated.
    dirty: bool,
}

const VS_SRC: &str = r#"
#if !defined(GL_ES) && __VERSION__ >= 140
#define I in
#define O out
#define V(x) x
#else
#define I attribute
#define O varying
#define V(x) vec3(x)
#endif
#ifdef GL_ES
precision mediump float;
#endif
uniform vec2 u_screen_size;
I vec2 a_pos;
I vec4 a_srgba; // 0-255 sRGB
I vec2 a_tc;
O vec4 v_rgba;
O vec2 v_tc;
// 0-1 linear  from  0-255 sRGB
vec3 linear_from_srgb(vec3 srgb) {
  bvec3 cutoff = lessThan(srgb, vec3(10.31475));
  vec3 lower = srgb / vec3(3294.6);
  vec3 higher = pow((srgb + vec3(14.025)) / vec3(269.025), vec3(2.4));
  return mix(higher, lower, V(cutoff));
}
vec4 linear_from_srgba(vec4 srgba) {
  return vec4(linear_from_srgb(srgba.rgb), srgba.a / 255.0);
}
void main() {
  gl_Position = vec4(2.0 * a_pos.x / u_screen_size.x - 1.0, 1.0 - 2.0 * a_pos.y / u_screen_size.y, 0.0, 1.0);
  // egui encodes vertex colors in gamma spaces, so we must decode the colors here:
  v_rgba = linear_from_srgba(a_srgba);
  v_tc = a_tc;
}
"#;

const FS_SRC: &str = r#"
#ifdef GL_ES
precision mediump float;
#endif
uniform sampler2D u_sampler;
#if defined(GL_ES) || __VERSION__ < 140
varying vec4 v_rgba;
varying vec2 v_tc;
#else
in vec4 v_rgba;
in vec2 v_tc;
out vec4 f_color;
#endif
#ifdef GL_ES
// 0-255 sRGB  from  0-1 linear
vec3 srgb_from_linear(vec3 rgb) {
  bvec3 cutoff = lessThan(rgb, vec3(0.0031308));
  vec3 lower = rgb * vec3(3294.6);
  vec3 higher = vec3(269.025) * pow(rgb, vec3(1.0 / 2.4)) - vec3(14.025);
  return mix(higher, lower, vec3(cutoff));
}
vec4 srgba_from_linear(vec4 rgba) {
  return vec4(srgb_from_linear(rgba.rgb), 255.0 * rgba.a);
}
#if __VERSION__ < 300
// 0-1 linear  from  0-255 sRGB
vec3 linear_from_srgb(vec3 srgb) {
  bvec3 cutoff = lessThan(srgb, vec3(10.31475));
  vec3 lower = srgb / vec3(3294.6);
  vec3 higher = pow((srgb + vec3(14.025)) / vec3(269.025), vec3(2.4));
  return mix(higher, lower, vec3(cutoff));
}
vec4 linear_from_srgba(vec4 srgba) {
  return vec4(linear_from_srgb(srgba.rgb), srgba.a / 255.0);
}
#endif
#endif
#ifdef GL_ES
void main() {
#if __VERSION__ < 300
  // We must decode the colors, since WebGL doesn't come with sRGBA textures:
  vec4 texture_rgba = linear_from_srgba(texture2D(u_sampler, v_tc) * 255.0);
#else
  // The texture is set up with `SRGB8_ALPHA8`, so no need to decode here!
  vec4 texture_rgba = texture2D(u_sampler, v_tc);
#endif
  /// Multiply vertex color with texture color (in linear space).
  gl_FragColor = v_rgba * texture_rgba;
  // We must gamma-encode again since WebGL doesn't support linear blending in the framebuffer.
  gl_FragColor = srgba_from_linear(v_rgba * texture_rgba) / 255.0;
  // WebGL doesn't support linear blending in the framebuffer,
  // so we apply this hack to at least get a bit closer to the desired blending:
  gl_FragColor.a = pow(gl_FragColor.a, 1.6); // Empiric nonsense
}
#else
void main() {
  // The texture sampler is sRGB aware, and OpenGL already expects linear rgba output
  // so no need for any sRGB conversions here:
#if __VERSION__ < 140
  gl_FragColor = v_rgba * texture2D(u_sampler, v_tc);
#else
  f_color = v_rgba * texture(u_sampler, v_tc);
#endif
}
#endif
"#;

pub struct Painter {
    vertex_array: gl::GLuint,
    program: gl::GLuint,
    index_buffer: gl::GLuint,
    pos_buffer: gl::GLuint,
    tc_buffer: gl::GLuint,
    color_buffer: gl::GLuint,
    canvas_width: u32,
    canvas_height: u32,
    vert_shader: gl::GLuint,
    frag_shader: gl::GLuint,
    user_textures: HashMap<u64, UserTexture>,
    managed_textures: HashMap<u64, UserTexture>,
}

pub fn compile_shader(src: &str, ty: gl::GLenum) -> gl::GLuint {
    let shader;
    unsafe {
        shader = gl::CreateShader(ty);
        // Attempt to compile the shader
        let c_str = std::ffi::CString::new(src.as_bytes()).unwrap();
        gl::ShaderSource(shader, 1, &c_str.as_ptr(), std::ptr::null());
        gl::CompileShader(shader);

        // Get the compile status
        let mut status = gl::FALSE as gl::GLint;
        gl::GetShaderiv(shader, gl::COMPILE_STATUS, &mut status);

        // Fail on error
        if status != (gl::TRUE as gl::GLint) {
            let mut len = 0;
            gl::GetShaderiv(shader, gl::INFO_LOG_LENGTH, &mut len);
            let mut buf = Vec::with_capacity(len as usize);
            buf.set_len((len as usize) - 1); // subtract 1 to skip the trailing null character
            gl::GetShaderInfoLog(
                shader,
                len,
                std::ptr::null_mut(),
                buf.as_mut_ptr() as *mut gl::GLchar,
            );
            panic!(
                "{}",
                std::str::from_utf8(&buf).expect("ShaderInfoLog not valid utf8")
            );
        }
    }
    shader
}

pub fn link_program(vs: gl::GLuint, fs: gl::GLuint) -> gl::GLuint {
    unsafe {
        let program = gl::CreateProgram();
        gl::AttachShader(program, vs);
        gl::AttachShader(program, fs);
        gl::LinkProgram(program);
        // Get the link status
        let mut status = gl::FALSE as gl::GLint;
        gl::GetProgramiv(program, gl::LINK_STATUS, &mut status);

        // Fail on error
        if status != (gl::TRUE as gl::GLint) {
            let mut len: gl::GLint = 0;
            gl::GetProgramiv(program, gl::INFO_LOG_LENGTH, &mut len);
            let mut buf = Vec::with_capacity(len as usize);
            buf.set_len((len as usize) - 1); // subtract 1 to skip the trailing null character
            gl::GetProgramInfoLog(
                program,
                len,
                std::ptr::null_mut(),
                buf.as_mut_ptr() as *mut gl::GLchar,
            );
            panic!(
                "{}",
                std::str::from_utf8(&buf).expect("ProgramInfoLog not valid utf8")
            );
        }
        program
    }
}

impl Painter {
    pub fn new(canvas_width: u32, canvas_height: u32) -> Painter {
        unsafe {
            let mut egui_texture = 0;
            // gl::load_with(|symbol| window.get_proc_address(symbol) as *const _);
            gl::GenTextures(1, &mut egui_texture);
            gl::BindTexture(gl::TEXTURE_2D, egui_texture);
            gl::TexParameteri(gl::TEXTURE_2D, gl::TEXTURE_WRAP_S, gl::CLAMP_TO_EDGE as i32);
            gl::TexParameteri(gl::TEXTURE_2D, gl::TEXTURE_WRAP_T, gl::CLAMP_TO_EDGE as i32);
            gl::TexParameteri(gl::TEXTURE_2D, gl::TEXTURE_MIN_FILTER, gl::LINEAR as i32);
            gl::TexParameteri(gl::TEXTURE_2D, gl::TEXTURE_MAG_FILTER, gl::LINEAR as i32);

            let vert_shader = compile_shader(VS_SRC, gl::VERTEX_SHADER);
            let frag_shader = compile_shader(FS_SRC, gl::FRAGMENT_SHADER);

            let program = link_program(vert_shader, frag_shader);
            let mut vertex_array = 0;
            let mut index_buffer = 0;
            let mut pos_buffer = 0;
            let mut tc_buffer = 0;
            let mut color_buffer = 0;
            gl::GenVertexArrays(1, &mut vertex_array);
            gl::BindVertexArray(vertex_array);
            gl::GenBuffers(1, &mut index_buffer);
            gl::GenBuffers(1, &mut pos_buffer);
            gl::GenBuffers(1, &mut tc_buffer);
            gl::GenBuffers(1, &mut color_buffer);

            Painter {
                vertex_array,
                program,
                canvas_width,
                canvas_height,
                index_buffer,
                pos_buffer,
                tc_buffer,
                color_buffer,
                vert_shader,
                frag_shader,
                user_textures: HashMap::new(),
                managed_textures: HashMap::new(),
            }
        }
    }

    pub fn adjust_size(&mut self, x: i32, y: i32) {
        self.canvas_width = x as u32;
        self.canvas_height = y as u32;
    }

    fn upload_managed_textures(&mut self) {
        unsafe {
            for (_, user_texture) in self.managed_textures.iter_mut() {
                if !user_texture.texture.is_none() && !user_texture.dirty {
                    continue;
                }
                let pixels = std::mem::take(&mut user_texture.pixels);

                if user_texture.texture.is_none() {
                    let mut gl_texture = 0;
                    gl::GenTextures(1, &mut gl_texture);
                    gl::BindTexture(gl::TEXTURE_2D, gl_texture);
                    gl::TexParameteri(gl::TEXTURE_2D, gl::TEXTURE_WRAP_S, gl::CLAMP_TO_EDGE as i32);
                    gl::TexParameteri(gl::TEXTURE_2D, gl::TEXTURE_WRAP_T, gl::CLAMP_TO_EDGE as i32);

                    if user_texture.filtering {
                        gl::TexParameteri(
                            gl::TEXTURE_2D,
                            gl::TEXTURE_MIN_FILTER,
                            gl::LINEAR as i32,
                        );
                        gl::TexParameteri(
                            gl::TEXTURE_2D,
                            gl::TEXTURE_MAG_FILTER,
                            gl::LINEAR as i32,
                        );
                    } else {
                        gl::TexParameteri(
                            gl::TEXTURE_2D,
                            gl::TEXTURE_MIN_FILTER,
                            gl::NEAREST as i32,
                        );
                        gl::TexParameteri(
                            gl::TEXTURE_2D,
                            gl::TEXTURE_MAG_FILTER,
                            gl::NEAREST as i32,
                        );
                    }
                    user_texture.texture = Some(gl_texture);
                } else {
                    gl::BindTexture(gl::TEXTURE_2D, user_texture.texture.unwrap());
                }

                let level = 0;
                let internal_format = gl::RGBA;
                let border = 0;
                let src_format = gl::RGBA;
                let src_type = gl::UNSIGNED_BYTE;

                gl::TexImage2D(
                    gl::TEXTURE_2D,
                    level,
                    internal_format as i32,
                    user_texture.size.0 as i32,
                    user_texture.size.1 as i32,
                    border,
                    src_format,
                    src_type,
                    pixels.as_ptr() as *const std::ffi::c_void,
                );

                user_texture.dirty = false;
            }
        }
    }

    fn upload_user_textures(&mut self) {
        unsafe {
            for (_, user_texture) in self.user_textures.iter_mut() {
                if !user_texture.texture.is_none() && !user_texture.dirty {
                    continue;
                }
                let pixels = std::mem::take(&mut user_texture.pixels);

                if user_texture.texture.is_none() {
                    let mut gl_texture = 0;
                    gl::GenTextures(1, &mut gl_texture);
                    gl::BindTexture(gl::TEXTURE_2D, gl_texture);
                    gl::TexParameteri(gl::TEXTURE_2D, gl::TEXTURE_WRAP_S, gl::CLAMP_TO_EDGE as i32);
                    gl::TexParameteri(gl::TEXTURE_2D, gl::TEXTURE_WRAP_T, gl::CLAMP_TO_EDGE as i32);

                    if user_texture.filtering {
                        gl::TexParameteri(
                            gl::TEXTURE_2D,
                            gl::TEXTURE_MIN_FILTER,
                            gl::LINEAR as i32,
                        );
                        gl::TexParameteri(
                            gl::TEXTURE_2D,
                            gl::TEXTURE_MAG_FILTER,
                            gl::LINEAR as i32,
                        );
                    } else {
                        gl::TexParameteri(
                            gl::TEXTURE_2D,
                            gl::TEXTURE_MIN_FILTER,
                            gl::NEAREST as i32,
                        );
                        gl::TexParameteri(
                            gl::TEXTURE_2D,
                            gl::TEXTURE_MAG_FILTER,
                            gl::NEAREST as i32,
                        );
                    }
                    user_texture.texture = Some(gl_texture);
                } else {
                    gl::BindTexture(gl::TEXTURE_2D, user_texture.texture.unwrap());
                }

                let level = 0;
                let internal_format = gl::RGBA;
                let border = 0;
                let src_format = gl::RGBA;
                let src_type = gl::UNSIGNED_BYTE;

                gl::TexImage2D(
                    gl::TEXTURE_2D,
                    level,
                    internal_format as i32,
                    user_texture.size.0 as i32,
                    user_texture.size.1 as i32,
                    border,
                    src_format,
                    src_type,
                    pixels.as_ptr() as *const std::ffi::c_void,
                );

                user_texture.dirty = false;
            }
        }
    }

    fn get_texture(&self, texture_id: egui::TextureId) -> gl::GLuint {
        match texture_id {
            egui::TextureId::User(id) => {
                let id = id as usize;
                assert!(id < self.user_textures.len());
                let texture = self
                    .user_textures
                    .get(&(id as u64))
                    .expect("Trying to retrieve unregistered user texture");
                texture.texture.expect("Should have been uploaded")
            }
            egui::TextureId::Managed(id) => {
                let id = id as usize;
                assert!(id < self.managed_textures.len());
                let texture = self
                    .managed_textures
                    .get(&(id as u64))
                    .expect("Trying to retrieve unregistered managed texture");
                texture.texture.expect("Should have been uploaded")
            }
        }
    }

    pub fn free_texture_delta(&mut self, f: Vec<egui::TextureId>) {
        for i in f {
            match i {
                egui::TextureId::User(id) => {
                    self.user_textures.remove(&id);
                }
                egui::TextureId::Managed(id) => {
                    self.managed_textures.remove(&id);
                }
            }
        }
    }

    pub fn set_texture_delta(
        &mut self,
        s: egui::epaint::ahash::AHashMap<
            egui::TextureId,
            egui::epaint::ImageDelta,
            egui::epaint::ahash::RandomState,
        >,
    ) {
        for i in s.keys() {
            let imdelta = s.get(i).unwrap();
            match i {
                egui::TextureId::User(id) => {
                    let pixels = match &imdelta.image {
                        egui::epaint::image::ImageData::Color(c) => c.pixels.clone(),
                        egui::epaint::image::ImageData::Font(c) => c.srgba_pixels(1.0).collect(),
                    };
                    let size = match &imdelta.image {
                        egui::epaint::image::ImageData::Color(c) => c.size,
                        egui::epaint::image::ImageData::Font(c) => c.size,
                    };
                    assert_eq!(size[0] * size[1], pixels.len());

                    let mut upixels: Vec<u8> = Vec::with_capacity(pixels.len() * 4);
                    for srgba in pixels {
                        upixels.push(srgba.r());
                        upixels.push(srgba.g());
                        upixels.push(srgba.b());
                        upixels.push(srgba.a());
                    }
                    if let Some([from, to]) = imdelta.pos {
                        let mut to_update = self.user_textures.get_mut(id).unwrap();
                        let mut out = to_update.pixels[0..(from - 1) * 4].to_vec();
                        out.append(&mut upixels);
                        out.append(&mut to_update.pixels[to * 4..].to_vec());
                        to_update.pixels = out;
                    } else {
                        //let id = egui::TextureId::User(self.user_textures.len() as u64);
                        self.user_textures.insert(
                            *id,
                            UserTexture {
                                size: (size[0], size[1]),
                                pixels: upixels,
                                texture: None,
                                filtering: true,
                                dirty: true,
                            },
                        );
                    }
                }
                egui::TextureId::Managed(id) => {
                    let pixels = match &imdelta.image {
                        egui::epaint::image::ImageData::Color(c) => c.pixels.clone(),
                        egui::epaint::image::ImageData::Font(c) => c.srgba_pixels(1.0).collect(),
                    };
                    let size = match &imdelta.image {
                        egui::epaint::image::ImageData::Color(c) => c.size,
                        egui::epaint::image::ImageData::Font(c) => c.size,
                    };
                    assert_eq!(size[0] * size[1], pixels.len());

                    let mut upixels: Vec<u8> = Vec::with_capacity(pixels.len() * 4);
                    let mut oc = vec![];
                    for srgba in pixels {
                        upixels.push(srgba.r());
                        upixels.push(srgba.g());
                        upixels.push(srgba.b());
                        upixels.push(srgba.a());
                        oc.push(srgba.r());
                    }
                    if let Some([from, to]) = imdelta.pos {
                        let mut to_update = self.managed_textures.get_mut(id).unwrap();
                        let mut out = to_update.pixels[0..(from - 1) * 4].to_vec();
                        out.append(&mut upixels);
                        out.append(&mut to_update.pixels[to * 4..].to_vec());
                        to_update.pixels = out;
                    } else {
                        self.managed_textures.insert(
                            *id,
                            UserTexture {
                                size: (size[0], size[1]),
                                pixels: upixels,
                                texture: None,
                                filtering: true,
                                dirty: true,
                            },
                        );
                    }
                }
            }
        }
    }

    pub fn paint_jobs(
        &mut self,
        meshes: Vec<egui::ClippedPrimitive>,
        pixels_per_point: f32,
        delta: egui::TexturesDelta,
    ) {
        // self.upload_egui_texture(egui_texture);

        self.set_texture_delta(delta.set);

        self.upload_user_textures();
        self.upload_managed_textures();

        unsafe {
            //Let OpenGL know we are dealing with SRGB colors so that it
            //can do the blending correctly. Not setting the framebuffer
            //leads to darkened, oversaturated colors.
            gl::Enable(gl::FRAMEBUFFER_SRGB);

            gl::Enable(gl::SCISSOR_TEST);
            gl::Enable(gl::BLEND);
            gl::BlendFunc(gl::ONE, gl::ONE_MINUS_SRC_ALPHA); // premultiplied alpha
            gl::UseProgram(self.program);
            gl::ActiveTexture(gl::TEXTURE0);

            let u_screen_size = std::ffi::CString::new("u_screen_size").unwrap();
            let u_screen_size_ptr = u_screen_size.as_ptr();
            let u_screen_size_loc = gl::GetUniformLocation(self.program, u_screen_size_ptr);
            let screen_size_pixels =
                egui::vec2(self.canvas_width as f32, self.canvas_height as f32);
            let screen_size_points = screen_size_pixels / pixels_per_point;
            gl::Uniform2f(
                u_screen_size_loc,
                screen_size_points.x,
                screen_size_points.y,
            );
            let u_sampler = std::ffi::CString::new("u_sampler").unwrap();
            let u_sampler_ptr = u_sampler.as_ptr();
            let u_sampler_loc = gl::GetUniformLocation(self.program, u_sampler_ptr);
            gl::Uniform1i(u_sampler_loc, 0);
            gl::Viewport(0, 0, self.canvas_width as i32, self.canvas_height as i32);

            for egui::ClippedPrimitive {
                clip_rect,
                primitive,
            } in meshes
            {
                if let egui::epaint::Primitive::Mesh(mesh) = primitive {
                    gl::BindTexture(gl::TEXTURE_2D, self.get_texture(mesh.texture_id));

                    let clip_min_x = pixels_per_point * clip_rect.min.x;
                    let clip_min_y = pixels_per_point * clip_rect.min.y;
                    let clip_max_x = pixels_per_point * clip_rect.max.x;
                    let clip_max_y = pixels_per_point * clip_rect.max.y;
                    let clip_min_x = clip_min_x.clamp(0.0, screen_size_pixels.x);
                    let clip_min_y = clip_min_y.clamp(0.0, screen_size_pixels.y);
                    let clip_max_x = clip_max_x.clamp(clip_min_x, screen_size_pixels.x);
                    let clip_max_y = clip_max_y.clamp(clip_min_y, screen_size_pixels.y);
                    let clip_min_x = clip_min_x.round() as i32;
                    let clip_min_y = clip_min_y.round() as i32;
                    let clip_max_x = clip_max_x.round() as i32;
                    let clip_max_y = clip_max_y.round() as i32;

                    //scissor Y coordinate is from the bottom
                    gl::Scissor(
                        clip_min_x,
                        self.canvas_height as i32 - clip_max_y,
                        clip_max_x - clip_min_x,
                        clip_max_y - clip_min_y,
                    );

                    self.paint_mesh(&mesh);
                    gl::Disable(gl::SCISSOR_TEST);
                } else {
                    eprintln!("Primitive callbacks are currently not implemented. ");
                    todo!()
                }
            }
            gl::Disable(gl::FRAMEBUFFER_SRGB);
        }
        self.free_texture_delta(delta.free);
    }

    pub fn cleanup(&self) {
        unsafe {
            gl::DeleteProgram(self.program);
            gl::DeleteShader(self.vert_shader);
            gl::DeleteShader(self.frag_shader);
            gl::DeleteBuffers(1, &self.pos_buffer);
            gl::DeleteBuffers(1, &self.tc_buffer);
            gl::DeleteBuffers(1, &self.color_buffer);
            gl::DeleteBuffers(1, &self.index_buffer);
            gl::DeleteVertexArrays(1, &self.vertex_array);
        }
    }

    fn paint_mesh(&self, mesh: &egui::Mesh) {
        debug_assert!(mesh.is_valid());
        let indices: Vec<u16> = mesh.indices.iter().map(|idx| *idx as u16).collect();

        let mut positions: Vec<f32> = Vec::with_capacity(2 * mesh.vertices.len());
        let mut tex_coords: Vec<f32> = Vec::with_capacity(2 * mesh.vertices.len());
        for v in &mesh.vertices {
            positions.push(v.pos.x);
            positions.push(v.pos.y);
            tex_coords.push(v.uv.x);
            tex_coords.push(v.uv.y);
        }

        let mut colors: Vec<u8> = Vec::with_capacity(4 * mesh.vertices.len());
        for v in &mesh.vertices {
            colors.push(v.color[0]);
            colors.push(v.color[1]);
            colors.push(v.color[2]);
            colors.push(v.color[3]);
        }

        unsafe {
            gl::BindVertexArray(self.vertex_array);
            gl::BindBuffer(gl::ELEMENT_ARRAY_BUFFER, self.index_buffer);
            gl::BufferData(
                gl::ELEMENT_ARRAY_BUFFER,
                (indices.len() * std::mem::size_of::<u16>()) as gl::GLsizeiptr,
                //mem::transmute(&indices.as_ptr()),
                indices.as_ptr() as *const gl::types::GLvoid,
                gl::STREAM_DRAW,
            );

            // --------------------------------------------------------------------
            gl::BindBuffer(gl::ARRAY_BUFFER, self.pos_buffer);
            gl::BufferData(
                gl::ARRAY_BUFFER,
                (positions.len() * std::mem::size_of::<f32>()) as gl::GLsizeiptr,
                //mem::transmute(&positions.as_ptr()),
                positions.as_ptr() as *const gl::types::GLvoid,
                gl::STREAM_DRAW,
            );

            let a_pos = std::ffi::CString::new("a_pos").unwrap();
            let a_pos_ptr = a_pos.as_ptr();
            let a_pos_loc = gl::GetAttribLocation(self.program, a_pos_ptr);
            assert!(a_pos_loc >= 0);
            let a_pos_loc = a_pos_loc as u32;

            let stride = 0;
            gl::VertexAttribPointer(a_pos_loc, 2, gl::FLOAT, gl::FALSE, stride, std::ptr::null());
            gl::EnableVertexAttribArray(a_pos_loc);

            // --------------------------------------------------------------------

            gl::BindBuffer(gl::ARRAY_BUFFER, self.tc_buffer);
            gl::BufferData(
                gl::ARRAY_BUFFER,
                (tex_coords.len() * std::mem::size_of::<f32>()) as gl::GLsizeiptr,
                //mem::transmute(&tex_coords.as_ptr()),
                tex_coords.as_ptr() as *const gl::types::GLvoid,
                gl::STREAM_DRAW,
            );

            let a_tc = std::ffi::CString::new("a_tc").unwrap();
            let a_tc_ptr = a_tc.as_ptr();
            let a_tc_loc = gl::GetAttribLocation(self.program, a_tc_ptr);
            assert!(a_tc_loc >= 0);
            let a_tc_loc = a_tc_loc as u32;

            let stride = 0;
            gl::VertexAttribPointer(a_tc_loc, 2, gl::FLOAT, gl::FALSE, stride, std::ptr::null());
            gl::EnableVertexAttribArray(a_tc_loc);

            // --------------------------------------------------------------------
            gl::BindBuffer(gl::ARRAY_BUFFER, self.color_buffer);
            gl::BufferData(
                gl::ARRAY_BUFFER,
                (colors.len() * std::mem::size_of::<u8>()) as gl::GLsizeiptr,
                //mem::transmute(&colors.as_ptr()),
                colors.as_ptr() as *const gl::types::GLvoid,
                gl::STREAM_DRAW,
            );

            let a_srgba = std::ffi::CString::new("a_srgba").unwrap();
            let a_srgba_ptr = a_srgba.as_ptr();
            let a_srgba_loc = gl::GetAttribLocation(self.program, a_srgba_ptr);
            assert!(a_srgba_loc >= 0);
            let a_srgba_loc = a_srgba_loc as u32;

            let stride = 0;
            gl::VertexAttribPointer(
                a_srgba_loc,
                4,
                gl::UNSIGNED_BYTE,
                gl::FALSE,
                stride,
                std::ptr::null(),
            );
            gl::EnableVertexAttribArray(a_srgba_loc);

            // --------------------------------------------------------------------

            gl::DrawElements(
                gl::TRIANGLES,
                indices.len() as i32,
                gl::UNSIGNED_SHORT,
                std::ptr::null(),
            );
        }
    }
}

impl Drop for Painter {
    fn drop(&mut self) {
        self.cleanup();
    }
}

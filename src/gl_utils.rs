use glad_gl::gl;
use std::ffi::CStr;
use std::ffi::CString;
use std::os::raw::c_char;

static VERTEX_SHADER: &'static str = r"
#version 130

attribute vec2 Position;
attribute vec2 UV;
attribute vec4 Color;
out vec2 Frag_UV;
out vec4 Frag_Color;

void main()
{
    Frag_UV = UV;
    Frag_Color = Color;
    gl_Position = vec4(Position.xy,0,1);
}
";

static FRAGMENT_SHADER: &'static str = r"
#version 130

uniform sampler2D Texture;
in vec2 Frag_UV;
in vec4 Frag_Color;
out vec4 Out_Color;

void main()
{
    Out_Color = vec4(1,0,0,1);
}
";

pub unsafe fn load_shaders() -> gl::GLuint {
    let vertex_shader_id = gl::CreateShader(gl::VERTEX_SHADER);
    let fragment_shader_id = gl::CreateShader(gl::FRAGMENT_SHADER);

    let mut result: gl::GLint = 0;
    let mut info_log_length: gl::GLint = 0;

    gl::ShaderSource(
        vertex_shader_id,
        1,
        &(CString::new(VERTEX_SHADER).unwrap().into_raw() as *const i8) as *const *const i8,
        std::ptr::null(),
    );
    gl::CompileShader(vertex_shader_id);

    gl::GetShaderiv(vertex_shader_id, gl::COMPILE_STATUS, &mut result as *mut _);
    gl::GetShaderiv(
        vertex_shader_id,
        gl::INFO_LOG_LENGTH,
        &mut info_log_length as *mut _,
    );
    if info_log_length > 0 {
        let mut info_log: Vec<c_char> = Vec::with_capacity(info_log_length as usize);
        info_log.resize(info_log_length as usize, 0);
        gl::GetShaderInfoLog(
            vertex_shader_id,
            info_log_length,
            std::ptr::null_mut(),
            info_log.as_mut_ptr(),
        );
        println!(
            "vertex shader log : {}",
            CStr::from_ptr(info_log.as_ptr()).to_str().unwrap()
        );
    }

    gl::ShaderSource(
        fragment_shader_id,
        1,
        &(CString::new(FRAGMENT_SHADER).unwrap().into_raw() as *const i8) as *const *const i8,
        std::ptr::null(),
    );
    gl::CompileShader(fragment_shader_id);

    gl::GetShaderiv(
        fragment_shader_id,
        gl::COMPILE_STATUS,
        &mut result as *mut _,
    );
    gl::GetShaderiv(
        fragment_shader_id,
        gl::INFO_LOG_LENGTH,
        &mut info_log_length as *mut _,
    );
    if info_log_length > 0 {
        let mut info_log: Vec<c_char> = Vec::with_capacity(info_log_length as usize);
        info_log.resize(info_log_length as usize, 0);
        gl::GetShaderInfoLog(
            fragment_shader_id,
            info_log_length,
            std::ptr::null_mut(),
            info_log.as_mut_ptr(),
        );
        println!(
            "fragment shader log : {}",
            CStr::from_ptr(info_log.as_ptr()).to_str().unwrap()
        );
    }

    let program_id = gl::CreateProgram();
    gl::AttachShader(program_id, vertex_shader_id);
    gl::AttachShader(program_id, fragment_shader_id);
    gl::LinkProgram(program_id);

    gl::GetProgramiv(program_id, gl::LINK_STATUS, &mut result as *mut _);
    gl::GetShaderiv(
        fragment_shader_id,
        gl::INFO_LOG_LENGTH,
        &mut info_log_length as *mut _,
    );
    if info_log_length > 0 {
        let mut info_log: Vec<c_char> = Vec::with_capacity(info_log_length as usize);
        info_log.resize(info_log_length as usize, 0);
        gl::GetProgramInfoLog(
            program_id,
            info_log_length,
            std::ptr::null_mut(),
            info_log.as_mut_ptr(),
        );
        println!(
            "program log : {}",
            CStr::from_ptr(info_log.as_ptr()).to_str().unwrap()
        );
    }

    gl::DetachShader(program_id, vertex_shader_id);
    gl::DetachShader(program_id, fragment_shader_id);
    gl::DeleteShader(vertex_shader_id);
    gl::DeleteShader(fragment_shader_id);

    program_id
}

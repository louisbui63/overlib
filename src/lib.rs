#[cfg(target_family = "unix")]
use libloading::os::unix::Symbol;

use std::ffi::c_void;
use std::ffi::CString;
use std::sync::Mutex;

use glad_gl::gl;

mod gl_utils;
mod painter;

#[macro_use]
extern crate lazy_static;

lazy_static! {
    static ref EGUI_CTX: egui::Context = egui::Context::default();
    static ref GLX: Glx = Glx::new();
    static ref CURRENT_FEATURES: Mutex<Vec<(gl::GLuint, bool)>> = Mutex::new(vec![]);
    static ref PAINTER: Mutex<painter::Painter> = Mutex::new(painter::Painter::new(800, 800));
}

static mut MUST_INIT: bool = true;

static mut PROGRAM_ID: gl::GLuint = 0;

static PIXELS_PER_POINT: f32 = 1.;

struct Glx {
    _lib: libloading::Library,
    swap_buffers: Symbol<unsafe extern "C" fn(*mut c_void, *mut c_void)>,
    get_proc_address: Symbol<unsafe extern "C" fn(*const c_void) -> *mut c_void>,
}

impl Glx {
    fn new() -> Self {
        unsafe {
            let lib = libloading::Library::new("libGL.so.1").unwrap();
            let swap_buffers = (lib.get(b"glXSwapBuffers").unwrap()
                as libloading::Symbol<unsafe extern "C" fn(*mut c_void, *mut c_void)>)
                .into_raw();
            let get_proc_address = (lib.get(b"glXGetProcAddress").unwrap()
                as libloading::Symbol<unsafe extern "C" fn(*const c_void) -> *mut c_void>)
                .into_raw();

            Self {
                _lib: lib,
                swap_buffers,
                get_proc_address,
            }
        }
    }
}

unsafe fn init(glx: &Glx) {
    gl::load(|e| (glx.get_proc_address)(CString::new(e).unwrap().into_raw() as *const c_void));
    PROGRAM_ID = gl_utils::load_shaders();
    MUST_INIT = false;
}

static VERTICES: [gl::GLfloat; 6] = [-1., -1., 1., -1., 0., 1.];
static mut VERTEX_BUFFER: gl::GLuint = 0;

#[deny(non_snake_case)]
#[no_mangle]
pub unsafe extern "C" fn glXSwapBuffers(dpy: *mut c_void, drawable: *mut c_void) {
    let glx = &GLX;

    if MUST_INIT {
        init(glx);

        gl::GenBuffers(1, &mut VERTEX_BUFFER as *mut _);
        gl::BindBuffer(gl::ARRAY_BUFFER, VERTEX_BUFFER);
        gl::BufferData(
            gl::ARRAY_BUFFER,
            (4 * 6) as isize,
            &VERTICES as *const _ as *const _,
            gl::STATIC_DRAW,
        );
    }

    let mut max_texture_size: i32 = std::mem::zeroed();
    gl::GetIntegerv(gl::MAX_TEXTURE_SIZE, &mut max_texture_size as *mut i32);

    let mut viewport: [i32; 4] = std::mem::zeroed();
    gl::GetIntegerv(gl::VIEWPORT, &mut viewport as *mut _);

    let inputs = egui::RawInput {
        screen_rect: Some(egui::Rect {
            min: egui::Pos2 {
                x: viewport[0] as f32,
                y: viewport[1] as f32,
            },
            max: egui::Pos2 {
                x: viewport[2] as f32,
                y: viewport[3] as f32,
            },
        }),
        pixels_per_point: Some(PIXELS_PER_POINT),
        max_texture_side: Some(max_texture_size as usize),
        time: None,
        predicted_dt: 1. / 60.,
        modifiers: egui::Modifiers::NONE,
        events: vec![],
        hovered_files: vec![],
        dropped_files: vec![],
    };

    let full_output = EGUI_CTX.run(inputs, ui_fn);

    set_required_features();

    let mut program = 0;
    gl::GetIntegerv(gl::CURRENT_PROGRAM, &mut program as *mut _ as *mut _);
    // gl::UseProgram(PROGRAM_ID);

    PAINTER
        .lock()
        .unwrap()
        .adjust_size(viewport[2], viewport[3]);
    PAINTER.lock().unwrap().paint_jobs(
        EGUI_CTX.tessellate(full_output.shapes),
        PIXELS_PER_POINT,
        full_output.textures_delta,
    );
    gl::UseProgram(program);

    (glx.swap_buffers)(dpy, drawable);

    restore_features();
}

fn ui_fn(ctx: &egui::Context) {
    egui::Window::new("TEST")
        .resize(|r| r.auto_sized())
        .show(ctx, |ui| ui.label("other"));
    // egui::Window::new("TEST2")
    //     .resize(|r| r.auto_sized())
    //     .show(ctx, |ui| ui.label("other"));
}

unsafe fn set_required_features() {
    CURRENT_FEATURES.lock().unwrap().clear();
    if gl::IsEnabled(gl::DEPTH_TEST) != 0 {
        CURRENT_FEATURES
            .lock()
            .unwrap()
            .push((gl::DEPTH_TEST, true));
        gl::Disable(gl::DEPTH_TEST);
    }
    if gl::IsEnabled(gl::SCISSOR_TEST) == 0 {
        CURRENT_FEATURES
            .lock()
            .unwrap()
            .push((gl::SCISSOR_TEST, false));
        gl::Enable(gl::SCISSOR_TEST);
    }
    if gl::IsEnabled(gl::BLEND) != 0 {
        CURRENT_FEATURES.lock().unwrap().push((gl::BLEND, true));
        gl::Disable(gl::BLEND);
    }
    if gl::IsEnabled(gl::CULL_FACE) != 0 {
        CURRENT_FEATURES.lock().unwrap().push((gl::CULL_FACE, true));
        gl::Disable(gl::CULL_FACE);
    }
}

unsafe fn restore_features() {
    for (feature, state) in CURRENT_FEATURES.lock().unwrap().to_vec() {
        (if state { gl::Enable } else { gl::Disable })(feature);
    }
}

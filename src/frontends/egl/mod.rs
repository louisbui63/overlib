use std::ffi::c_void;
use std::ffi::CString;
use std::sync::Mutex;

use glad_gl::gl;

lazy_static! {
    static ref EGL: Egl = Egl::new();
    static ref CURRENT_FEATURES: Mutex<Vec<(gl::GLuint, bool)>> = Mutex::new(vec![]);
    static ref PAINTER: Mutex<crate::backends::opengl::painter::Painter> =
        Mutex::new(crate::backends::opengl::painter::Painter::new(800, 800));
}

static mut MUST_INIT: bool = true;

static PIXELS_PER_POINT: f32 = 1.;

struct Egl {
    _lib: dlopen::raw::Library,
    swap_buffers: unsafe extern "C" fn(*mut c_void, *mut c_void) -> libc::c_uint,
    get_proc_address: unsafe extern "C" fn(*const c_void) -> *mut c_void,
}

impl Egl {
    fn new() -> Self {
        unsafe {
            let lib = dlopen::raw::Library::open("libEGL.so.1").unwrap();
            let swap_buffers = lib
                .symbol_cstr(&std::ffi::CString::new("eglSwapBuffers").unwrap())
                .unwrap();
            let get_proc_address = lib
                .symbol_cstr(&std::ffi::CString::new("eglGetProcAddress").unwrap())
                .unwrap();

            Self {
                _lib: lib,
                swap_buffers,
                get_proc_address,
            }
        }
    }
}

unsafe fn init(glx: &Egl) {
    gl::load(|e| (glx.get_proc_address)(CString::new(e).unwrap().into_raw() as *const c_void));
    MUST_INIT = false;
}

#[no_mangle]
pub unsafe extern "C" fn overlib_egl_swap_buffers(
    dpy: *mut c_void,
    drawable: *mut c_void,
) -> std::os::raw::c_uint {
    eglSwapBuffers(dpy, drawable)
}

#[deny(non_snake_case)]
#[no_mangle]
pub unsafe extern "C" fn eglSwapBuffers(
    dpy: *mut c_void,
    drawable: *mut c_void,
) -> std::os::raw::c_uint {
    let egl = &EGL;

    if MUST_INIT {
        init(egl);
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

    let full_output = crate::EGUI_CTX.run(inputs, crate::ui_fn);

    set_required_features();

    let mut program = 0;
    gl::GetIntegerv(gl::CURRENT_PROGRAM, &mut program as *mut _ as *mut _);

    PAINTER
        .lock()
        .unwrap()
        .adjust_size(viewport[2], viewport[3]);
    PAINTER.lock().unwrap().paint_jobs(
        crate::EGUI_CTX.tessellate(full_output.shapes),
        PIXELS_PER_POINT,
        full_output.textures_delta,
    );
    gl::UseProgram(program);

    let out = (egl.swap_buffers)(dpy, drawable);

    restore_features();

    out
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

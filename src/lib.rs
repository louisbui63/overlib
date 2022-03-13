#[cfg(target_family = "unix")]
use libloading::os::unix::Symbol;

use std::ffi::c_void;
use std::ffi::CString;

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

#[deny(non_snake_case)]
#[no_mangle]
pub unsafe extern "C" fn glXSwapBuffers(dpy: *mut c_void, drawable: *mut c_void) {
    let glx = Glx::new();

    let gl = glow::Context::from_loader_function(|s| {
        (glx.get_proc_address)(CString::new(s).unwrap().into_raw() as *const _)
    });

    (glx.swap_buffers)(dpy, drawable);
}

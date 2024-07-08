// https://github.com/flightlessmango/MangoHud/blob/master/src/hook_dlsym.cpp

use std::ffi::c_void;
use std::os::raw::c_char;

extern "C" {
    fn real_dlsym(handle: *mut c_void, symbol: *const c_char) -> *mut c_void;
}

// this part should actually be compiled separately and

#[no_mangle]
pub unsafe extern "C" fn dlsym(handle: *mut c_void, symbol: *const c_char) -> *mut c_void {
    let real = real_dlsym(handle, symbol);
    if real.is_null() {
        return real;
    }
    if let Ok(soname) = std::ffi::CStr::from_ptr(symbol).to_str() {
        match soname {
            "glXSwapBuffers" => {
                let a = real_dlsym(
                    libc::RTLD_NEXT,
                    b"overlib_glx_swap_buffers\0" as *const [u8] as *const [i8] as _,
                );
                println!("{:p}", a);
                return a;
            }
            "glXGetProcAddress" => {
                let a = real_dlsym(
                    libc::RTLD_NEXT,
                    b"overlib_glx_get_proc_address\0" as *const [u8] as *const [i8] as _,
                );
                println!("{:p}", a);
                return a;
            }
            "glXGetProcAddressARB" => {
                let a = real_dlsym(
                    libc::RTLD_NEXT,
                    b"overlib_glx_get_proc_address_arb\0" as *const [u8] as *const [i8] as _,
                );
                println!("{:p}", a);
                return a;
            }
            "eglSwapBuffers" => {
                let a = real_dlsym(
                    libc::RTLD_NEXT,
                    b"overlib_egl_swap_buffers\0" as *const [u8] as *const [i8] as _,
                );
                println!("{:p}", a);
                return a;
            }
            _ => {}
        }
    }
    real
}

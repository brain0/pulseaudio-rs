use libc;
use libpulse_sys::*;
use std::ffi::CStr;

pub fn strerror_ref(error: libc::c_int) -> &'static CStr {
    unsafe { CStr::from_ptr(pa_strerror(error)) }
}

pub fn strerror(error: libc::c_int) -> String {
    strerror_ref(error).to_string_lossy().into_owned()
}

//! Functions to convert pulseaudio error codes to error messages.

use libc;
use libpulse_sys::*;
use std::ffi::CStr;

/// Converts a pulseaudio error code into a static string reference.
pub fn strerror_ref(error: libc::c_int) -> &'static CStr {
    unsafe { CStr::from_ptr(pa_strerror(error)) }
}

/// Converts a pulseaudio error code into an owned string.
pub fn strerror(error: libc::c_int) -> String {
    strerror_ref(error).to_string_lossy().into_owned()
}

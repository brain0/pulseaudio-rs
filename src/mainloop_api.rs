//! Rust trait for a pulseaudio mainloop abstraction.
use libpulse_sys::pa_mainloop_api;

/// Trait for types that provide a pulseaudio mainloop abstraction.
///
/// A user of the asynchronous pulseaudio API must hook into a mainloop.
/// This requires a [mainloop abstraction](https://freedesktop.org/software/pulseaudio/doxygen/async.html#mainloop_sec).
/// Types that implement such a mainloop abstraction must implement this
/// trait.
///
/// Types that implement this trait must Implement `Clone`, since many objects
/// in this library hold a copy of the mainloop API to ensure that it lives
/// long enough.
///
/// # Safety
///
/// Implementers of this trait must ensure that all callbacks in the structure returned
/// by [`get_api`] are set and that they have defined behaviour when they are called from
/// `libpulse`.
///
/// [`get_api`]: #tymethod.get_api
pub unsafe trait PaMainLoopApi: Clone {
    /// Returns a raw pointer to a mainloop API structure.
    fn get_api(&self) -> *mut pa_mainloop_api;
}

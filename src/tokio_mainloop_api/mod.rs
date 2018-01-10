//! Implementation of [`PaMainLoopApi`] for `tokio_core`.
//!
//! [`PaMainLoopApi`]: ../mainloop_api/trait.PaMainLoopApi.html
mod api_impl;

use libpulse_sys::*;
use std::rc::Rc;
use tokio_core::reactor::Handle;

use self::api_impl::TokioMainLoopApiImpl;
use super::mainloop_api::PaMainLoopApi;

/// Implementation of [`PaMainLoopApi`] for `tokio_core`.
///
/// This implementation can be used with any mainloop that uses
/// `tokio_core::rector::Core`.
///
/// Note that while pulseaudio deferred events are being handled,
/// this implementation blocks all other events until all deferred
/// events are disabled. This behaviour closely resembles the behaviour
/// of pulseaudio's own mainloop.
///
/// [`PaMainLoopApi`]: ../mainloop_api/trait.PaMainLoopApi.html
#[derive(Clone)]
pub struct PaMainLoopApiTokio {
    intern: Rc<TokioMainLoopApiImpl>,
}

impl PaMainLoopApiTokio {
    /// Creates a new pulseaudio mainloop API.
    ///
    /// Requires a handle to a `tokio_core::rector::Core`.
    pub fn new(handle: &Handle) -> PaMainLoopApiTokio {
        PaMainLoopApiTokio { intern: api_impl::new(handle) }
    }
}

unsafe impl PaMainLoopApi for PaMainLoopApiTokio {
    fn get_api(&self) -> *mut pa_mainloop_api {
        self.intern.get_api()
    }
}

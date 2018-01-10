//! Pulseaudio context.
mod state;

use libc;
use libpulse_sys::*;
use std::ffi::CStr;
use std::ptr::{null, null_mut};

use mainloop_api::PaMainLoopApi;
use refcount::RefCounted;

pub use self::state::PaContextState;
pub use self::state::PaContextStateStream;

/// The basic object for a connection to a pulseaudio server.
///
/// A context multiplexes commands, data streams and events through a single channel.
/// There is no need for more than one context per application, unless connections to multiple servers are needed.
#[derive(Clone)]
pub struct PaContext<M: PaMainLoopApi> {
    raw: RefCounted<pa_context>,
    mainloop_api: M,
    state_cb_receivers: state::StateCallbackReceivers,
}

impl<M: PaMainLoopApi> PaContext<M> {
    /// Creates a new pulseaudion context.
    ///
    /// # Arguments
    ///
    /// * `api`: Reference to a pulseaudio mainloop API.
    /// * `name`: Application name.
    pub fn new(api: &M, name: &CStr) -> PaContext<M> {
        let raw;
        unsafe {
            let ptr = pa_context_new(api.get_api(), name.as_ptr());
            assert!(ptr != null_mut());
            raw = RefCounted::new(ptr);
        }
        let state_cb_receivers = state::StateCallbackReceivers::new(raw.clone());
        PaContext {
            raw: raw,
            mainloop_api: api.clone(),
            state_cb_receivers,
        }
    }

    /// Returns the error number of the last failed operation.
    ///
    /// This number can be converted into a human-readable string using the
    /// [`strerror`](../error/fn.strerror.html) and [`strerror_ref`](../error/fn.strerror_ref.html)
    /// functions.
    pub fn errno(&self) -> libc::c_int {
        unsafe { pa_context_errno(self.raw.get()) }
    }
    
    /// Returns the current context status.
    pub fn get_state(&self) -> PaContextState {
        state::get_state(&self.raw)
    }

    /// Returns a stream that notifies of context status changes.
    pub fn get_state_stream(&self) -> PaContextStateStream {
        self.state_cb_receivers.get_stream()
    }

    /// Connect the context to the specified server.
    ///
    /// If server is None, connect to the default server. This routine may but will not always return synchronously on error.
    /// Use the stream returned by [`get_state_stream`](#method.get_state_stream) to be notified when the connection is established.
    pub fn connect(&self, server: Option<&CStr>) -> bool {
        unsafe { pa_context_connect(self.raw.get(), match server { Some(s) => s.as_ptr(), None => null() }, PA_CONTEXT_NOAUTOSPAWN, null())  >= 0 }
    }

    /// Terminate the context connection immediately.
    pub fn disconnect(&self) {
        unsafe { pa_context_disconnect(self.raw.get()) }
    }
}

pa_refcountable!(pa_context, pa_context_ref, pa_context_unref);

mod state;

use libc;
use libpulse_sys::*;
use std::ffi::CStr;
use std::ptr::{null, null_mut};

use mainloop_api::MainLoopApi;
use refcount::RefCounted;

pub use self::state::PaContextState;
pub use self::state::PaContextStateStream;

#[derive(Clone)]
pub struct PaContext<M: MainLoopApi> {
    raw: RefCounted<pa_context>,
    mainloop_api: M,
    state_cb_receivers: state::StateCallbackReceivers,
}

impl<M: MainLoopApi> PaContext<M> {
    pub fn new(m: &M, name: &CStr) -> PaContext<M> {
        let raw;
        unsafe {
            let ptr = pa_context_new(m.get_api(), name.as_ptr());
            assert!(ptr != null_mut());
            raw = RefCounted::new(ptr);
        }
        let state_cb_receivers = state::StateCallbackReceivers::new(raw.clone());
        PaContext {
            raw: raw,
            mainloop_api: m.clone(),
            state_cb_receivers,
        }
    }

    pub fn errno(&self) -> libc::c_int {
        unsafe { pa_context_errno(self.raw.get()) }
    }
    
    pub fn get_state(&self) -> PaContextState {
        state::get_state(&self.raw)
    }

    pub fn get_state_stream(&self) -> PaContextStateStream {
        self.state_cb_receivers.get_stream()
    }

    pub fn connect(&self, server: Option<&CStr>) -> bool {
        unsafe { pa_context_connect(self.raw.get(), match server { Some(s) => s.as_ptr(), None => null() }, PA_CONTEXT_NOAUTOSPAWN, null())  >= 0 }
    }

    pub fn disconnect(&self) {
        unsafe { pa_context_disconnect(self.raw.get()) }
    }
}

unsafe impl super::refcount::RefCountable for pa_context {
    fn decref(ptr: *mut Self) {
        unsafe { pa_context_unref(ptr) };
    }

    fn incref(ptr: *mut Self) {
        unsafe { pa_context_ref(ptr) };
    }
}

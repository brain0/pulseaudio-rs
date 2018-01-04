mod api_impl;

use libpulse_sys::*;
use futures::unsync::oneshot;
use libc;
use std::rc::Rc;
use tokio_core::reactor::Handle;

use self::api_impl::TokioMainLoopApiImpl;
use super::mainloop_api::MainLoopApi;

#[derive(Clone)]
pub struct TokioMainLoopApi {
    intern: Rc<TokioMainLoopApiImpl>,
}

impl TokioMainLoopApi {
    pub fn new(quit: Option<oneshot::Sender<libc::c_int>>, handle: &Handle) -> TokioMainLoopApi {
        TokioMainLoopApi { intern: api_impl::new(quit, handle) }
    }
}

unsafe impl MainLoopApi for TokioMainLoopApi {
    fn get_api(&self) -> *mut pa_mainloop_api {
        self.intern.get_api()
    }
}

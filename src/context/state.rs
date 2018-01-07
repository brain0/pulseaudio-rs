use future_pubsub::unsync as pubsub;
use futures::prelude::*;
use libc;
use libpulse_sys::*;
use refcount::RefCounted;
use std::ptr::null_mut;
use std::rc::Rc;
use std::cell::RefCell;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PaContextState {
    Unconnected,
    Connecting,
    Authorizing,
    SettingName,
    Ready,
    Failed,
    Terminated
}

impl PaContextState {
    pub fn new(s: pa_context_state_t) -> Result<PaContextState, ()> {
        match s {
            PA_CONTEXT_UNCONNECTED => Ok(PaContextState::Unconnected),
            PA_CONTEXT_CONNECTING => Ok(PaContextState::Connecting),
            PA_CONTEXT_AUTHORIZING => Ok(PaContextState::Authorizing),
            PA_CONTEXT_SETTING_NAME => Ok(PaContextState::SettingName),
            PA_CONTEXT_READY => Ok(PaContextState::Ready),
            PA_CONTEXT_FAILED => Ok(PaContextState::Failed),
            PA_CONTEXT_TERMINATED => Ok(PaContextState::Terminated),
            _ => Err(())
        }
    }
}

pub fn get_state(raw: &RefCounted<pa_context>) -> PaContextState {
    PaContextState::new(unsafe { pa_context_get_state(raw.get()) }).unwrap()
}

struct StateCallbackReceiversImpl {
    raw_ctx: RefCounted<pa_context>,
    sender: pubsub::UnboundedSender<PaContextState>,
    receiver: RefCell<pubsub::UnboundedReceiver<PaContextState>>,
}

#[derive(Clone)]
pub struct StateCallbackReceivers(Rc<StateCallbackReceiversImpl>);

pub struct PaContextStateStream(pubsub::UnboundedReceiver<PaContextState>);

impl StateCallbackReceivers {
    pub fn new(raw_ctx: RefCounted<pa_context>) -> StateCallbackReceivers {
        let (sender, receiver) = pubsub::unbounded();
        let ret = StateCallbackReceivers(Rc::new(StateCallbackReceiversImpl {
            raw_ctx,
            sender,
            receiver: RefCell::new(receiver),
        }));
        unsafe { pa_context_set_state_callback(ret.0.raw_ctx.get(), Some(notify_state_cb), &*(ret.0) as *const _ as *mut libc::c_void) };
        ret
    }

    pub fn get_stream(&self) -> PaContextStateStream {
        PaContextStateStream(self.0.receiver.borrow().clone())
    }
}

impl StateCallbackReceiversImpl {
    fn send(&self) {
        self.sender.unbounded_send(get_state(&self.raw_ctx)).unwrap();
        match self.receiver.borrow_mut().poll() {
            Ok(Async::Ready(Some(_))) => (),
            _ => panic!("Could not drain dummy receiver"),
        }
    }
}

extern "C" fn notify_state_cb(_ctx: *mut pa_context, userdata: *mut libc::c_void) {
    assert!(userdata != null_mut());
    let data = unsafe { &*(userdata as *const StateCallbackReceiversImpl) };
    data.send();
}

impl Stream for PaContextStateStream {
    type Item = PaContextState;
    type Error = ();

    fn poll(&mut self) -> Poll<Option<PaContextState>, ()> {
        match self.0.poll() {
            Ok(Async::Ready(Some(s))) => Ok(Async::Ready(Some(*s))),
            Ok(Async::Ready(None)) => Ok(Async::Ready(None)),
            Ok(Async::NotReady) => Ok(Async::NotReady),
            Err(()) => Err(()),
        }
    }
}

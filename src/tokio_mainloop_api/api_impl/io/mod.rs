mod flags;
mod io_event_stream;

use futures::{future, task};
use libc;
use libpulse_sys::*;
use mio::Ready;
use std::cell::Cell;
use std::collections::HashMap;
use std::mem;
use std::os::unix::io::RawFd;

use self::io_event_stream::IoEventStreams;
use super::TokioMainLoopApiImpl;
use super::external_reference::ExternalReference;

struct IoEvent {
    fd: RawFd,
    cb: pa_io_event_cb_t,
    destroy_cb: Cell<pa_io_event_destroy_cb_t>,
    userdata: *mut libc::c_void,
    reference: ExternalReference,
}

pub struct Io (HashMap<usize, IoEvent>, IoEventStreams);

impl Io {
    pub fn new() -> Io {
        Io(HashMap::new(), IoEventStreams::new())
    }

    pub fn free_all(&mut self, api: *mut pa_mainloop_api) {
        mem::replace(&mut self.0, HashMap::new()).into_iter().for_each(|(_, v)| {
            if let Some(cb) = v.destroy_cb.get() {
                unsafe { cb(api, v.reference.as_ptr(), v.userdata) };
            }
        });
    }

    pub fn spawn(&mut self,
                 fd: RawFd,
                 events: pa_io_event_flags_t,
                 cb: pa_io_event_cb_t,
                 userdata: *mut libc::c_void,
                 data: &TokioMainLoopApiImpl) -> *mut pa_io_event {
        let reference = ExternalReference::new(data.weak_ref());
        let reference_ptr = reference.as_ptr();

        self.0.insert(reference_ptr as usize, IoEvent {
            fd,
            cb,
            destroy_cb: Cell::new(None),
            userdata,
            reference
        });
        self.1.set(data, fd, reference_ptr as usize, flags::pulse_to_mio(events));
        reference_ptr
    }

    unsafe fn event_fn<F, T>(e: *mut pa_io_event, f: F) -> T where F: FnOnce(&TokioMainLoopApiImpl, usize) -> T {
        ExternalReference::run(e, |data| f(data, e as usize))
    }

    pub unsafe fn enable(e: *mut pa_io_event,
                         events: pa_io_event_flags_t) {
        Self::event_fn(e, |data, index| data.io.borrow().enable_impl(data, index, events));
    }

    fn enable_impl(&self, data: &TokioMainLoopApiImpl, index: usize, events: pa_io_event_flags_t) {
        let ev = self.0.get(&index).unwrap();
        self.1.set(data, ev.fd, index, flags::pulse_to_mio(events));
    }

    pub unsafe fn free(e: *mut pa_io_event) {
        if task::in_task() {
            Self::event_fn(e, |data, _| data.io.borrow().free_impl(data, e))
        } else {
            ExternalReference::run(e, |data| {
                Self::call_destroy_cb(data, e);
            })
        }
    }

    fn free_impl(&self, data: &TokioMainLoopApiImpl, e: *mut pa_io_event) {
        let ev = self.0.get(&(e as usize)).unwrap();
        self.1.set(data, ev.fd, e as usize, Ready::empty());
        let weak = data.weak_ref();
        data.handle.spawn(future::lazy(move || {
            if let Some(data) = weak.upgrade() {
                Self::call_destroy_cb(&*data, e);
            }
            Ok(())
        }));
    }

    fn call_destroy_cb(data: &TokioMainLoopApiImpl, e: *mut pa_io_event) {
        let io;
        {
            let mut d = data.io.borrow_mut();
            io = d.0.remove(&(e as usize));
        }
        if let Some(io) = io {
            if let Some(cb) = io.destroy_cb.get() {
                unsafe { cb(data.get_api(), e, io.userdata) };
            }
        }
    }

    pub unsafe fn set_destroy_cb(e: *mut pa_io_event,
                                 cb: pa_io_event_destroy_cb_t) {
        Self::event_fn(e, |data, index| data.io.borrow().set_destroy_cb_impl(index, cb))
    }

    fn set_destroy_cb_impl(&self, index: usize, cb: pa_io_event_destroy_cb_t) {
        let ev = self.0.get(&index).unwrap();
        ev.destroy_cb.set(cb);
    }
}

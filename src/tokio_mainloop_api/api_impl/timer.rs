use libc;
use libpulse_sys::*;
use std::collections::HashMap;
use std::mem;
use std::ptr::null_mut;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio_core::reactor::Timeout;
use futures::prelude::*;
use futures::{future, task};
use futures::unsync::oneshot;

use super::TokioMainLoopApiImpl;
use super::completion_future::CompletionFuture;
use super::external_reference::ExternalReference;

struct Timer {
    cancel: Option<oneshot::Sender<CompletionFuture>>,
    cb: pa_time_event_cb_t,
    destroy_cb: pa_time_event_destroy_cb_t,
    userdata: *mut libc::c_void,
    reference: ExternalReference,
}

pub struct Timers(HashMap<usize, Timer>);

impl Timers {
    pub fn new() -> Timers {
        Timers(HashMap::new())
    }

    fn spawn_timeout_handler(data: &TokioMainLoopApiImpl,
                             reference: *mut pa_time_event,
                             tv: libc::timeval) -> oneshot::Sender<CompletionFuture> {
        assert!(tv.tv_usec < 1000000);
        let timeout = match (UNIX_EPOCH + Duration::new(tv.tv_sec as u64, tv.tv_usec as u32 * 1000)).duration_since(SystemTime::now()) {
            Ok(t) => t,
            Err(_) => Duration::new(0, 0),
        };
        let weak = data.weak_ref();
        let (send, rec) = oneshot::channel();
        let f = Timeout::new(timeout, &data.handle).expect("Unable to create timeout")
                .select2(rec)
                .then(move |r| {
                    match r {
                        Ok(future::Either::A((_, mut rec))) | Err(future::Either::A((_, mut rec))) => {
                            if let Ok(Async::Ready(f)) = rec.poll() {
                                f
                            } else {
                                drop(rec);
                                if let Some(p) = weak.upgrade() {
                                    let mut cb = None;
                                    let mut userdata = null_mut();

                                    {
                                        let timers = p.timers.borrow();
                                        if let Some(timer) = timers.0.get(&(reference as usize)) {
                                            cb = timer.cb;
                                            userdata = timer.userdata;
                                        }
                                    }

                                    if let Some(cb) = cb {
                                        if reference != null_mut() {
                                            unsafe { cb(p.get_api(), reference, &tv, userdata) };
                                            return CompletionFuture::Ok
                                        }
                                    }
                                }
                                CompletionFuture::Err
                            }
                        },
                        Ok(future::Either::B((f, _))) => f,
                        Err(future::Either::B((_, _))) => CompletionFuture::Err,
                    }
                });
        data.handle.spawn(f);
        send
    }

    pub fn spawn(&mut self,
                 tv: Option<&libc::timeval>,
                 cb: pa_time_event_cb_t,
                 userdata: *mut libc::c_void,
                 data: &TokioMainLoopApiImpl) -> *mut pa_time_event {
        let reference = ExternalReference::new(data.weak_ref());
        let reference_ptr = reference.as_ptr();

        let cancel = if let Some(tv) = tv {
            Some(Self::spawn_timeout_handler(data,
                                             reference_ptr,
                                             *tv))
        } else {
            None
        };

        self.0.insert(reference_ptr as usize, Timer {
            cancel,
            cb,
            destroy_cb: None,
            userdata,
            reference,
        });

        reference_ptr
    }

    unsafe fn event_fn<F, T>(e: *mut pa_time_event, f: F) -> T where F: FnOnce(&TokioMainLoopApiImpl, usize) -> T {
        ExternalReference::run(e, |data| f(data, e as usize))
    }

    pub unsafe fn restart(e: *mut pa_time_event, tv: Option<&libc::timeval>) {
        Self::event_fn(e, |data, index| data.timers.borrow_mut().restart_impl(data, index, tv))
    }

    fn restart_impl(&mut self, data: &TokioMainLoopApiImpl, index: usize, tv: Option<&libc::timeval>) {
        let timer = self.0.get_mut(&index).unwrap();
        if let Some(s) = timer.cancel.take() {
            drop(s.send(CompletionFuture::Ok));
        }

        if let Some(tv) = tv {
            timer.cancel = Some(Self::spawn_timeout_handler(data,
                                                            timer.reference.as_ptr(),
                                                            *tv));
        };
    }
    
    pub unsafe fn free(e: *mut pa_time_event) {
        Self::event_fn(e, |data, index| data.timers.borrow_mut().free_impl(data, index))
    }

    fn free_impl(&mut self, data: &TokioMainLoopApiImpl, index: usize) {
        if task::in_task() {
            let weak = data.weak_ref();
            let f = future::lazy(move || {
                if let Some(p) = weak.upgrade() {
                    let v;
                    {
                        let mut timers = p.timers.borrow_mut();
                        v = timers.0.remove(&index);
                    }
                    if let Some(v) = v {
                        if let Some(cb) = v.destroy_cb {
                            unsafe { cb(p.get_api(), v.reference.as_ptr(), v.userdata) };
                        }
                    }
                }
                Ok(())
            });
            let timer = self.0.get_mut(&index).unwrap();
            if let Some(s) = timer.cancel.take() {
                if let Err(f) = s.send(CompletionFuture::Boxed(Box::new(f))) {
                    data.handle.spawn(f);
                }
            } else {
                data.handle.spawn(f);
            }
        } else {
            let v = self.0.remove(&index);
            if let Some(v) = v {
                if let Some(cb) = v.destroy_cb {
                    unsafe { cb(data.get_api(), v.reference.as_ptr(), v.userdata) };
                }
            }
        }
    }

    pub fn free_all(&mut self, api: *mut pa_mainloop_api) {
        mem::replace(&mut self.0, HashMap::new()).into_iter().for_each(|(_, v)| {
            if let Some(cb) = v.destroy_cb {
                unsafe { cb(api, v.reference.as_ptr(), v.userdata) };
            }
        });
    }

    pub unsafe fn set_destroy_cb(e: *mut pa_time_event,
                                 cb: pa_time_event_destroy_cb_t) {
        Self::event_fn(e, |data, index| data.timers.borrow_mut().set_destroy_cb_impl(index, cb))
    }

    fn set_destroy_cb_impl(&mut self, index: usize, cb: pa_time_event_destroy_cb_t) {
        let timer = self.0.get_mut(&index).unwrap();
        timer.destroy_cb = cb;
    }
}

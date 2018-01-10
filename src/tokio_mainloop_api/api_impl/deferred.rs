use futures::prelude::*;
use futures::task::{self, Task};
use libc;
use libpulse_sys::*;
use std::cell::Cell;
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Weak;
use std::mem;
use std::ops::DerefMut;

use super::TokioMainLoopApiImpl;
use super::external_reference::ExternalReference;

struct DeferredEvent {
    dead: Cell<bool>,
    active: Cell<bool>,
    cb: pa_defer_event_cb_t,
    destroy_cb: Cell<pa_defer_event_destroy_cb_t>,
    userdata: *mut libc::c_void,
    reference: ExternalReference,
}

pub struct Deferred {
    events: HashMap<usize, DeferredEvent>,
    new_events: RefCell<HashMap<usize, DeferredEvent>>,
    task: Option<Task>,
}

impl Deferred {
    pub fn new() -> Deferred {
        Deferred{ events: HashMap::new(), new_events: RefCell::new(HashMap::new()), task: None }
    }

    pub fn free_all(&mut self, api: *mut pa_mainloop_api) {
        mem::replace(&mut self.events, HashMap::new()).into_iter().for_each(|(_, v)| {
            if let Some(cb) = v.destroy_cb.get() {
                unsafe { cb(api, v.reference.as_ptr(), v.userdata) };
            }
        });
    }

    pub fn launch(data: &TokioMainLoopApiImpl) {
        data.handle.spawn(DeferredEventFuture(data.weak_ref()));
    }

    pub fn add(data: &TokioMainLoopApiImpl, cb: pa_defer_event_cb_t, userdata: *mut libc::c_void) -> *mut pa_defer_event {
        let reference = ExternalReference::new(data.weak_ref());
        let reference_ptr = reference.as_ptr();
        let event = DeferredEvent {
            dead: Cell::new(false),
            active: Cell::new(true),
            cb,
            destroy_cb: Cell::new(None),
            userdata: userdata,
            reference: reference,
        };
        if let Ok(mut deferred) = data.deferred.try_borrow_mut() {
            deferred.events.insert(reference_ptr as usize, event);
            if let Some(ref task) = deferred.task {
                task.notify();
            }
        } else {
            let deferred = data.deferred.borrow();
            let mut new_events = deferred.new_events.borrow_mut();
            new_events.insert(reference_ptr as usize, event);
        }
        reference_ptr
    }

    unsafe fn event_fn<F>(e: *mut pa_defer_event, f: F) where F: FnOnce(&Self, &DeferredEvent) {
        ExternalReference::run(e, |data| {
            let d = data.deferred.borrow();
            if let Some(deferred) = d.events.get(&(e as usize)) {
                f(&*d, deferred);
            } else {
                let new_events = d.new_events.borrow();
                if let Some(deferred) = new_events.get(&(e as usize)) {
                    f(&*d, deferred)
                }
            }
        })
    }

    pub unsafe fn enable(e: *mut pa_defer_event, enabled: bool) {
        Self::event_fn(e, |data, ev| data.enable_impl(ev, enabled))
    }

    fn enable_impl(&self, ev: &DeferredEvent, enabled: bool) {
        let old = ev.active.replace(enabled);
        if let Some(ref task) = self.task {
            if old != enabled && ! task.will_notify_current() {
                task.notify();
            }
        }
    }

    pub unsafe fn free(e: *mut pa_defer_event) {
        if task::in_task() {
            Self::event_fn(e, |data, ev| data.free_impl(ev))
        } else {
            ExternalReference::run(e, |data| {
                let mut deferred;
                {
                    let mut d = data.deferred.borrow_mut();
                    deferred = d.events.remove(&(e as usize));
                    if deferred.is_none() {
                        let mut new_events = d.new_events.borrow_mut();
                        deferred = new_events.remove(&(e as usize));
                    }
                }
                if let Some(deferred) = deferred {
                    if let Some(cb) = deferred.destroy_cb.get() {
                        cb(data.get_api(), e, deferred.userdata);
                    }
                }
            })
        }
    }

    fn free_impl(&self, ev: &DeferredEvent) {
        ev.active.set(false);
        ev.dead.set(true);
        if let Some(ref task) = self.task {
            if ! task.will_notify_current() {
                task.notify();
            }
        }
    }

    pub unsafe fn set_destroy(e: *mut pa_defer_event, cb: pa_defer_event_destroy_cb_t) {
        Self::event_fn(e, |data, ev| data.set_destroy_impl(ev, cb))
    }

    fn set_destroy_impl(&self, ev: &DeferredEvent, cb: pa_defer_event_destroy_cb_t) {
        ev.destroy_cb.set(cb);
    }
}

struct DeferredEventFuture(Weak<TokioMainLoopApiImpl>);

impl Future for DeferredEventFuture {
    type Item = ();
    type Error = ();

    fn poll(&mut self) -> Poll<(), ()> {
        if let Some(data) = self.0.upgrade() {
            {
                let mut d = data.deferred.borrow_mut();
                if match d.task {
                    Some(ref task) => ! task.will_notify_current(),
                    None => true
                } {
                    d.task = Some(task::current());
                }
            }

            loop {
                let mut abort = true;

                {
                    let d = data.deferred.borrow();
                    for v in d.events.values() {
                        if v.active.get() {
                            unsafe { v.cb.unwrap()(data.get_api(), v.reference.as_ptr(), v.userdata) };
                            abort = false;
                        }
                    }
                }

                let dead_events: Vec<DeferredEvent>;
                {
                    let mut d = data.deferred.borrow_mut();
                    let d = d.deref_mut();

                    {
                        let new_events = d.new_events.get_mut();
                        if ! new_events.is_empty() {
                            let events = &mut d.events;
                            new_events.drain().for_each(|(k,v)|
                            {
                                if ! events.insert(k,v).is_none() {
                                    panic!("Inconsistent deferred events");
                                }
                            });
                            abort = false;
                        }
                    }

                    let dead_event_keys: Vec<usize> = d.events.iter()
                        .filter(|&(_, ref v)| v.dead.get())
                        .map(&|(&k, _)| k)
                        .collect();
                    dead_events = dead_event_keys.into_iter()
                        .map(|k| d.events.remove(&k).unwrap())
                        .collect();
                }

                if ! dead_events.is_empty() {
                    dead_events.into_iter().for_each(|v| {
                        if let Some(cb) = v.destroy_cb.get() {
                            unsafe { cb(data.get_api(), v.reference.as_ptr(), v.userdata) };
                        }
                    });
                    abort = false;
                }

                if abort {
                    return Ok(Async::NotReady);
                }
            }
        } else {
            Err(())
        }
    }
}
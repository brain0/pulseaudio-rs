mod completion_future;
mod external_reference;
mod deferred;
mod io;
mod timer;

use libc;
use libpulse_sys::*;
use futures::unsync::oneshot;
use std::cell::Cell;
use std::cell::RefCell;
use std::ptr::{null, null_mut};
use std::rc::Rc;
use std::rc::Weak;
use tokio_core::reactor::Handle;
use self::timer::Timers;
use self::deferred::Deferred;
use self::io::Io;

pub struct TokioMainLoopApiImpl {
    handle: Handle,
    weak_self_ref: RefCell<Option<Weak<TokioMainLoopApiImpl>>>,
    api: pa_mainloop_api,
    quit_notify: Cell<Option<oneshot::Sender<libc::c_int>>>,
    quitting: Cell<bool>,
    timers: RefCell<Timers>,
    deferred: RefCell<Deferred>,
    io: RefCell<Io>,
}

impl TokioMainLoopApiImpl {
    pub fn get_api(&self) -> *mut pa_mainloop_api {
        &self.api as *const _ as *mut _
    }

    fn weak_ref(&self) -> Weak<TokioMainLoopApiImpl> {
        self.weak_self_ref.borrow().as_ref().unwrap().clone()
    }

    fn quit(&self, retval: libc::c_int) {
        self.quitting.set(true);
        if let Some(f) = self.quit_notify.take() {
            f.send(retval).expect("Unable to send quit signal");
        }
    }
}

impl Drop for TokioMainLoopApiImpl {
    fn drop(&mut self) {
        self.deferred.borrow_mut().free_all(&mut self.api as *mut _);
        self.timers.borrow_mut().free_all(&mut self.api as *mut _);
        self.io.borrow_mut().free_all(&mut self.api as *mut _);
    }
}

pub fn new(quit_notify: Option<oneshot::Sender<libc::c_int>>, handle: &Handle) -> Rc<TokioMainLoopApiImpl> {
    let mut intern = Rc::new(TokioMainLoopApiImpl {
        handle: handle.clone(),
        api: pa_mainloop_api {
            userdata: null_mut(),
            io_new: Some(io_new_cb),
            io_enable: Some(io_enable_cb),
            io_free: Some(io_free_cb),
            io_set_destroy: Some(io_set_destroy_cb),
            time_new: Some(time_new_cb),
            time_restart: Some(time_restart_cb),
            time_free: Some(time_free_cb),
            time_set_destroy: Some(time_set_destroy_cb),
            defer_new: Some(defer_new_cb),
            defer_enable: Some(defer_enable_cb),
            defer_free: Some(defer_free_cb),
            defer_set_destroy: Some(defer_set_destroy_cb),
            quit: Some(quit_cb),
        },
        weak_self_ref: RefCell::new(None),
        quit_notify: Cell::new(quit_notify),
        quitting: Cell::new(false),
        timers: RefCell::new(Timers::new()),
        deferred: RefCell::new(Deferred::new()),
        io: RefCell::new(Io::new()),
    });

    {
        let intern_mut = Rc::get_mut(&mut intern).unwrap();
        let ptr = intern_mut as *mut TokioMainLoopApiImpl as *mut libc::c_void;
        intern_mut.api.userdata = ptr;
    }
    let intern_weak = Rc::downgrade(&intern);
    *intern.weak_self_ref.borrow_mut() = Some(intern_weak);
    Deferred::launch(&*intern);
    intern
}

unsafe fn run_api_function<T, F>(a: *mut pa_mainloop_api, f: F) -> T where F: Fn(&TokioMainLoopApiImpl) -> T {
    assert!(a != null_mut());
    assert!((*a).userdata != null_mut());
    f(&*((*a).userdata as *const TokioMainLoopApiImpl))
}

unsafe extern "C" fn io_new_cb(a: *mut pa_mainloop_api,
                        fd: libc::c_int,
                        events: pa_io_event_flags_t,
                        cb: pa_io_event_cb_t,
                        userdata: *mut libc::c_void) -> *mut pa_io_event {
    assert!(cb.is_some());
    assert!(fd >= 0);
    run_api_function(a, |data| data.io.borrow_mut().spawn(fd, events, cb, userdata, data))
}

unsafe extern "C" fn io_enable_cb(e: *mut pa_io_event,
                                  events: pa_io_event_flags_t) {
    assert!(e != null_mut());
    Io::enable(e, events)
}

unsafe extern "C" fn io_free_cb(e: *mut pa_io_event) {
    assert!(e != null_mut());
    Io::free(e)
}

unsafe extern "C" fn io_set_destroy_cb(e: *mut pa_io_event,
                                       cb: pa_io_event_destroy_cb_t) {
    assert!(e != null_mut());
    Io::set_destroy_cb(e, cb)
}

unsafe extern "C" fn time_new_cb(a: *mut pa_mainloop_api,
                                 tv: *const libc::timeval,
                                 cb: pa_time_event_cb_t,
                                 userdata: *mut libc::c_void) -> *mut pa_time_event {
    assert!(cb.is_some());
    run_api_function(a, |data| data.timers.borrow_mut().spawn(ref_from_ptr(tv), cb, userdata, data))
}

unsafe extern "C" fn time_restart_cb(e: *mut pa_time_event,
                                     tv: *const libc::timeval) {
    assert!(e != null_mut());
    Timers::restart(e, ref_from_ptr(tv));
}

unsafe extern "C" fn time_free_cb(e: *mut pa_time_event) {
    assert!(e != null_mut());
    Timers::free(e)
}

unsafe extern "C" fn time_set_destroy_cb(e: *mut pa_time_event,
                                         cb: pa_time_event_destroy_cb_t) {
    assert!(e != null_mut());
    Timers::set_destroy_cb(e, cb)
}

unsafe extern "C" fn defer_new_cb(a: *mut pa_mainloop_api,
                                  cb: pa_defer_event_cb_t,
                                  userdata: *mut libc::c_void) -> *mut pa_defer_event {
    assert!(cb.is_some());
    run_api_function(a, |data| Deferred::add(data, cb, userdata))
}

unsafe extern "C" fn defer_enable_cb(e: *mut pa_defer_event,
                                     b: libc::c_int) {
    assert!(e != null_mut());
    Deferred::enable(e, b != 0)
}

unsafe extern "C" fn defer_free_cb(e: *mut pa_defer_event) {
    assert!(e != null_mut());
    Deferred::free(e)
}

unsafe extern "C" fn defer_set_destroy_cb(e: *mut pa_defer_event,
                                          cb: pa_defer_event_destroy_cb_t) {
    assert!(e != null_mut());
    Deferred::set_destroy(e, cb)
}

unsafe extern "C" fn quit_cb(a: *mut pa_mainloop_api,
                             retval: libc::c_int) {
    run_api_function(a, |data| data.quit(retval))
}

unsafe fn ref_from_ptr<'a, T>(ptr: *const T) -> Option<&'a T> {
    if ptr == null() {
        None
    } else {
        Some(&*ptr)
    }
}

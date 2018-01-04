use libc;
use libpulse_sys::*;
use mio::{self, Evented, Ready};
use mio::unix::EventedFd;
use futures::prelude::*;
use futures::task::{self, Task};
use std::cell::RefCell;
use std::collections::HashMap;
use std::io::Result as IoResult;
use std::os::unix::io::RawFd;
use std::rc::{Rc, Weak};
use tokio_core::reactor::{Handle, PollEvented};

use super::flags;
use super::super::TokioMainLoopApiImpl;

struct IoEventStreamData {
    task: Option<Task>,
    events: HashMap<usize, Ready>,
}

struct IoEventStream {
    parent: Weak<TokioMainLoopApiImpl>,
    handle: Handle,
    fd: RawFd,
    poll_evented: Option<PollEvented<OwnedEventedFd>>,
    data: Option<Rc<RefCell<IoEventStreamData>>>,
}

pub struct IoEventStreams(RefCell<HashMap<RawFd, Weak<RefCell<IoEventStreamData>>>>);

impl IoEventStreams {
    pub fn new() -> IoEventStreams {
        IoEventStreams(RefCell::new(HashMap::new()))
    }

    fn spawn_new_stream(parent: &TokioMainLoopApiImpl, fd: RawFd, index: usize, events: Ready) -> Weak<RefCell<IoEventStreamData>> {
        let mut event_map = HashMap::new();
        event_map.insert(index, events);
        let data = Rc::new(RefCell::new(IoEventStreamData { task: None, events: event_map }));
        let ret = Rc::downgrade(&data);
        let weak = parent.weak_ref(); 
        parent.handle.spawn(IoEventStream {
            parent: parent.weak_ref(),
            handle: parent.handle.clone(),
            fd,
            poll_evented: Some(PollEvented::new(OwnedEventedFd(fd), &parent.handle).unwrap()),
            data: Some(data),
        }.for_each(move |(events, ready)| {
            if let Some(p) = weak.upgrade() {
                events.into_iter().for_each(|e| {
                    let cb;
                    let fd;
                    let ready_pulse;
                    let userdata;
                    {
                      let io = p.io.borrow();
                      if let Some(ev) = io.0.get(&e.0) {
                          cb = ev.cb;
                          fd = ev.fd;
                          ready_pulse = flags::mio_to_pulse(ready & e.1);
                          userdata = ev.userdata;
                      } else {
                          return ();
                      }
                    }
                    unsafe { cb.unwrap()(p.get_api(), e.0 as *mut pa_io_event, fd, ready_pulse, userdata) };
                });
                Ok(())
            } else {
                Err(())
            }
        }));
        ret
    }
    
    fn add_to_existing_stream(data: &mut IoEventStreamData, index: usize, events: Ready) {
        if events.is_empty() {
            data.events.remove(&index);
        } else {
            data.events.insert(index, events);
        }
    }

    pub fn set(&self, parent: &TokioMainLoopApiImpl, fd: RawFd, index: usize, events: Ready) {
        let mut fd_map = self.0.borrow_mut();
        let fd_map = &mut *fd_map;

        fd_map.retain(|_, weak| weak.upgrade().is_some());

        if let Some(weak) = fd_map.get_mut(&fd) {
            let data = weak.upgrade().unwrap();
            let mut data = data.borrow_mut();
            Self::add_to_existing_stream(&mut *data, index, events);
            if let Some(ref task) = data.task {
                if ! task.will_notify_current() {
                    task.notify();
                }
            }
            return ();
        }
        if ! events.is_empty() {
            fd_map.insert(fd, Self::spawn_new_stream(parent, fd, index, events));
        }
    }
}

impl IoEventStream {
    fn do_poll(&mut self) -> Async<Option<<Self as Stream>::Item>> {
        if !self.parent.upgrade().is_some() {
            return Async::Ready(None);
        }

        {
            let mut data = self.data.as_ref().unwrap().borrow_mut();
            if match data.task {
                Some(ref task) => ! task.will_notify_current(),
                None => true
            } {
                data.task = Some(task::current());
            }
        }

        let mut events = Ready::empty();
        self.data.as_ref().unwrap().borrow().events.iter().for_each(|(_, &m)| events |= m);
        if events.is_empty() {
            return Async::Ready(None);
        }

        let poll_evented = self.poll_evented.as_ref().unwrap();
        match poll_evented.poll_ready(events)
        {
            Async::Ready(mut ready) => {
                // Simulate level semantics by calling poll() on the file descriptor
                // and checking for actual readiness.
                let mut pollfd = libc::pollfd { fd: self.fd, events: flags::mio_to_poll(ready), revents: 0 };
                match unsafe { libc::poll(&mut pollfd, 1, 0) } {
                    //-1 => ???,
                    0 => ready = Ready::empty(),
                    1 => ready &= flags::poll_to_mio(pollfd.revents),
                    _ => panic!("Unexpected return value from poll()"),
                }

                if !(events & !Ready::writable()).is_empty() && (ready & !Ready::writable()).is_empty() {
                    poll_evented.need_read();
                }
                if !(events & Ready::writable()).is_empty() && (ready & Ready::writable()).is_empty() {
                    poll_evented.need_write();
                }
                if ready.is_empty() {
                    Async::NotReady
                } else {
                    let v = self.data.as_ref().unwrap().borrow().events.iter()
                        .filter(|&(_, m)| !(*m & ready).is_empty())
                        .map(|(i, m)| (*i, *m))
                        .collect(); 
                    Async::Ready(Some((v, ready)))
                }
            },
            Async::NotReady => return Async::NotReady,
        }
    }

    fn unregister(&mut self) -> Option<()> {
        self.poll_evented.take().map(|poll_evented| drop(poll_evented.deregister(&self.handle)))
    }
}

impl Stream for IoEventStream {
    type Item = (Vec<(usize, Ready)>, Ready);
    type Error = ();

    fn poll(&mut self) -> Poll<Option<Self::Item>, ()> {
        let res = self.do_poll();
        match res {
            Async::Ready(None) => {
                self.unregister().expect("Inconsistent state");
                self.data.take();
            },
            Async::NotReady | Async::Ready(Some(_)) => ()
        };
        Ok(res)
    }
}

impl Drop for IoEventStream {
    fn drop(&mut self) {
        self.unregister();
    }
}

struct OwnedEventedFd(RawFd);

impl Evented for OwnedEventedFd {
    fn register(&self, poll: &mio::Poll, token: mio::Token, interest: Ready, opts: mio::PollOpt) -> IoResult<()> {
        EventedFd(&self.0).register(poll, token, interest, opts)
    }

    fn reregister(&self, poll: &mio::Poll, token: mio::Token, interest: Ready, opts: mio::PollOpt) -> IoResult<()> {
        EventedFd(&self.0).reregister(poll, token, interest, opts)
    }

    fn deregister(&self, poll: &mio::Poll) -> IoResult<()> {
        EventedFd(&self.0).deregister(poll)
    }
}

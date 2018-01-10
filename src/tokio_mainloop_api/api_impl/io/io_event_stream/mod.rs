mod poll_evented_level;

use libpulse_sys::*;
use mio::Ready;
use futures::prelude::*;
use futures::task::{self, Task};
use std::cell::RefCell;
use std::collections::HashMap;
use std::os::unix::io::RawFd;
use std::rc::{Rc, Weak};

use self::poll_evented_level::PollEventedFdLevel;
use super::flags;
use super::super::TokioMainLoopApiImpl;

struct IoEventStreamData {
    task: Option<Task>,
    events: HashMap<usize, Ready>,
}

struct IoEventStream {
    parent: Weak<TokioMainLoopApiImpl>,
    poll: PollEventedFdLevel,
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
            poll: PollEventedFdLevel::new(fd, &parent.handle).unwrap(),
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

        match self.poll.poll_ready(events)
        {
            Async::Ready(ready) => {
                let v = self.data.as_ref().unwrap().borrow().events.iter()
                        .filter(|&(_, m)| !(*m & ready).is_empty())
                        .map(|(i, m)| (*i, *m))
                        .collect(); 
                Async::Ready(Some((v, ready)))
            },
            Async::NotReady => return Async::NotReady,
        }
    }
}

impl Stream for IoEventStream {
    type Item = (Vec<(usize, Ready)>, Ready);
    type Error = ();

    fn poll(&mut self) -> Poll<Option<Self::Item>, ()> {
        let res = self.do_poll();
        match res {
            Async::Ready(None) => {
                self.poll.unregister().expect("Inconsistent state");
                self.data.take();
            },
            Async::NotReady | Async::Ready(Some(_)) => ()
        };
        Ok(res)
    }
}


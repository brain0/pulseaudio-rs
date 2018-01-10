use futures::Async;
use libc;
use mio::{self, Evented, Ready};
use mio::unix::EventedFd;
use std::io::Result as IoResult;
use std::os::unix::io::RawFd;
use tokio_core::reactor::{Handle, PollEvented};

use super::super::flags;

pub struct PollEventedFdLevel {
    handle: Handle,
    fd: RawFd,
    poll_evented: Option<PollEvented<OwnedEventedFd>>,
}

impl PollEventedFdLevel {
    pub fn new(fd: RawFd, handle: &Handle) -> IoResult<PollEventedFdLevel> {
        Ok(PollEventedFdLevel {
            handle: handle.clone(),
            fd,
            poll_evented: Some(PollEvented::new(OwnedEventedFd(fd), handle)?),
        })
    }

    pub fn poll_ready(&self, mask: Ready) -> Async<Ready> {
        let poll_evented = self.poll_evented.as_ref().unwrap();
        match poll_evented.poll_ready(mask) {
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

                if !(mask & !Ready::writable()).is_empty() && (ready & !Ready::writable()).is_empty() {
                    poll_evented.need_read();
                }
                if !(mask & Ready::writable()).is_empty() && (ready & Ready::writable()).is_empty() {
                    poll_evented.need_write();
                }
                if ready.is_empty() {
                    Async::NotReady
                } else {
                    Async::Ready(ready)
                }
            },
            Async::NotReady => Async::NotReady,
        }
    }

    pub fn unregister(&mut self) -> Option<()> {
        self.poll_evented.take().map(|poll_evented| drop(poll_evented.deregister(&self.handle)))
    }
}

impl Drop for PollEventedFdLevel {
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

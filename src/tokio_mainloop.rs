use libc;
use futures::unsync::oneshot;
use futures::future;
use futures::Future;
use std::io;
use tokio_core::reactor::{Core, Handle, Remote};

pub struct TokioMainLoop {
    core: Core,
    quit: oneshot::Receiver<libc::c_int>,
}

impl TokioMainLoop {
    pub fn new() -> Result<(TokioMainLoop, oneshot::Sender<libc::c_int>), io::Error> {
        let core = Core::new()?;
        let (send, receive) = oneshot::channel();
        Ok((TokioMainLoop { core, quit: receive }, send))
    }

    pub fn handle(&self) -> Handle {
        self.core.handle()
    }

    pub fn remote(&self) -> Remote {
        self.core.remote()
    }

    pub fn run(self) -> Result<libc::c_int, ()> {
        let mut core = self.core;
        core.run(self.quit.or_else(|_| future::err(())))
    }
}

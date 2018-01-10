extern crate futures;
extern crate pulseaudio;
extern crate tokio_core;

use futures::prelude::*;
use futures::unsync::oneshot;
use pulseaudio::prelude::*;
use std::ffi::CString;
use std::time::Duration;
use tokio_core::reactor::{Core, Timeout};

fn xmain() -> Result<(), ()> {
    let mut core = Core::new().unwrap();
    let m = PaMainLoopApiTokio::new(&core.handle());
    let (quit_send, quit_receive) = oneshot::channel();
    let mut quit_send = Some(quit_send);

    let name = CString::new("RustPulseaudioTest").unwrap();
    let ctx = pulseaudio::context::PaContext::new(&m, &name);
    {
        let h = core.handle();
        let ctx = ctx.clone();
        core.handle().spawn(ctx.get_state_stream().for_each(move |s| {
            eprintln!("New state: {:?}", s);
            let err = ctx.errno();
            if err != 0 {
                eprintln!("Last error: {}: {}", err, pulseaudio::error::strerror(err));
            }
            match s {
                pulseaudio::context::PaContextState::Failed => { quit_send.take().unwrap().send(Err(())).unwrap(); },
                pulseaudio::context::PaContextState::Terminated => { quit_send.take().unwrap().send(Ok(())).unwrap(); },
                pulseaudio::context::PaContextState::Ready => {
                    let ctx = ctx.clone();
                    h.spawn(Timeout::new(Duration::from_secs(5), &h).unwrap().and_then(move |_| {
                        eprintln!("Disconnecting");
                        ctx.disconnect();
                        Ok(())
                    }).or_else(|_| Err(())));
                }
                _ => (),
            };
            Ok(())
        }));
    }
    assert!(ctx.connect(None));

    core.run(quit_receive).unwrap()
}

fn main() {
    std::process::exit(match xmain() {
        Ok(()) => { eprintln!("Exiting with success!"); 0 },
        Err(()) => { eprintln!("Exiting with error!"); -1 },
    });
}

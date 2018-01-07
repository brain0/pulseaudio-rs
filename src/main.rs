extern crate futures;
extern crate pulseaudio;
extern crate tokio_core;

use futures::prelude::*;
use pulseaudio::mainloop_api::*;
use pulseaudio::tokio_mainloop::TokioMainLoop;
use pulseaudio::tokio_mainloop_api::TokioMainLoopApi;
use std::ffi::CString;
use std::time::Duration;
use tokio_core::reactor::Timeout;

fn xmain() -> i32 {
    let (l, qh) = TokioMainLoop::new().unwrap();
    let m = TokioMainLoopApi::new(Some(qh), &l.handle());

    let name = CString::new("RustPulseaudioTest").unwrap();
    let ctx = pulseaudio::context::PaContext::new(&m, &name);
    {
        let h = l.handle();
        let ctx = ctx.clone();
        l.handle().spawn(ctx.get_state_stream().for_each(move |s| {
            eprintln!("New state: {:?}", s);
            let err = ctx.errno();
            if err != 0 {
                eprintln!("Last error: {}: {}", err, pulseaudio::error::strerror(err));
            }
            match s {
                pulseaudio::context::PaContextState::Failed => m.quit(1),
                pulseaudio::context::PaContextState::Terminated => m.quit(0),
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

    match l.run() {
        Ok(v) => {
            eprintln!("Stopped with return value {}", v);
            v
        },
        Err(()) => {
            eprintln!("Main loop exited with an error");
            -1
        }
    }
}

fn main() {
    let exit_code = xmain();
    std::process::exit(exit_code);
}

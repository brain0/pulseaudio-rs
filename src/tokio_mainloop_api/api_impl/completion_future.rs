use futures::prelude::*;

pub enum CompletionFuture {
    Ok,
    Err,
    Boxed(Box<Future<Item = (), Error = ()>>),
}

impl Future for CompletionFuture {
    type Item = ();
    type Error = ();

    fn poll(&mut self) -> Poll<(), ()> {
        match *self {
            CompletionFuture::Ok => Ok(Async::Ready(())),
            CompletionFuture::Err => Err(()),
            CompletionFuture::Boxed(ref mut f) => f.poll(),
        }
    }
}

#![warn(missing_docs)]
//! A safe abstraction of the asynchronous pulseaudio API
//!
//! This crate provides safe access to the asynchronous API of `libpulse`.
//! It includes a mainloop API abstraction based on `futures`, `mio` and
//! `tokio-core`.

extern crate futures;
extern crate libpulse_sys;
extern crate libc;
extern crate mio;
extern crate tokio_core;
extern crate future_pubsub;

#[macro_use]
mod refcount;
mod explicit_cleanup;
pub mod context;
pub mod error;
pub mod mainloop_api;
pub mod tokio_mainloop_api;

/// A "prelude" for crates using the `pulseaudio` crate.
pub mod prelude {
    #[doc(no_inline)]
    pub use mainloop_api::PaMainLoopApi;
    #[doc(no_inline)]
    pub use tokio_mainloop_api::PaMainLoopApiTokio;
    #[doc(no_inline)]
    pub use context::PaContext;
}

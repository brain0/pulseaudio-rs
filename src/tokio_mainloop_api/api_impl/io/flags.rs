use libc;
use libpulse_sys;
use mio::Ready;
use mio::unix::UnixReady;

pub fn pulse_to_mio(flags: libpulse_sys::pa_io_event_flags_t) -> Ready {
    let mut ret = Ready::empty();
    if flags & libpulse_sys::PA_IO_EVENT_INPUT != 0 {
        ret |= Ready::readable();
    }
    if flags & libpulse_sys::PA_IO_EVENT_OUTPUT != 0 {
        ret |= Ready::writable();
    }
    if flags & libpulse_sys::PA_IO_EVENT_HANGUP != 0 {
        ret |= UnixReady::hup();
    }
    if flags & libpulse_sys::PA_IO_EVENT_ERROR != 0 {
        ret |= UnixReady::error();
    }
    ret
}

pub fn mio_to_pulse(flags: Ready) -> libpulse_sys::pa_io_event_flags_t {
    let mut ret = libpulse_sys::PA_IO_EVENT_NULL;
    let flags = UnixReady::from(flags);
    if flags.is_readable() {
        ret |= libpulse_sys::PA_IO_EVENT_INPUT;
    }
    if flags.is_writable() {
        ret |= libpulse_sys::PA_IO_EVENT_OUTPUT;
    }
    if flags.is_hup() {
        ret |= libpulse_sys::PA_IO_EVENT_HANGUP;
    }
    if flags.is_error() {
        ret |= libpulse_sys::PA_IO_EVENT_ERROR;
    }
    ret
}

pub fn mio_to_poll(flags: Ready) -> libc::c_short {
    let mut ret = 0;
    let flags = UnixReady::from(flags);
    if flags.is_readable() {
        ret |= libc::POLLIN;
    }
    if flags.is_writable() {
        ret |= libc::POLLOUT;
    }
    if flags.is_hup() {
        ret |= libc::POLLHUP;
    }
    if flags.is_error() {
        ret |= libc::POLLERR;
    }
    ret
}

pub fn poll_to_mio(flags: libc::c_short) -> Ready {
    let mut ret = Ready::empty();
    if flags & libc::POLLIN != 0 {
        ret |= Ready::readable();
    }
    if flags & libc::POLLOUT != 0 {
        ret |= Ready::writable();
    }
    if flags & libc::POLLHUP != 0 {
        ret |= UnixReady::hup();
    }
    if flags & libc::POLLERR != 0 {
        ret |= UnixReady::error();
    }
    ret
}

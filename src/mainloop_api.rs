use libc;
use libpulse_sys::pa_mainloop_api;

pub unsafe trait MainLoopApi: Clone {
    fn get_api(&self) -> *mut pa_mainloop_api;

    fn quit(&self, retval: libc::c_int) {
        let api = self.get_api();
        unsafe { (*api).quit.unwrap()(api, retval) };
    }
}

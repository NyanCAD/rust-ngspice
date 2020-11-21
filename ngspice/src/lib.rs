use ngspice_sys::*;
use std::ffi::{CStr, CString};
use std::os::raw::{c_char, c_int, c_void};
use std::sync::Once;

static START: Once = Once::new();

struct NgSpice<C> {
    callbacks: Box<C>,
}

extern "C" fn dummy_controlled_exit(
    _arg1: c_int,
    _arg2: bool,
    _arg3: bool,
    _arg4: c_int,
    _arg5: *mut c_void,
) -> c_int {
    0
}

impl<C> Drop for NgSpice<C> {
    fn drop(&mut self) {
        unsafe {
            ngSpice_Init(
                None,
                None,
                Some(dummy_controlled_exit),
                None,
                None,
                None,
                std::ptr::null_mut(),
            );
        }
    }
}

impl<C: Callbacks> NgSpice<C> {
    fn new(c: C) -> NgSpice<C> {
        let ptr = Box::into_raw(Box::new(c));
        START.call_once(|| unsafe {
            ngSpice_Init(
                Some(send_char::<C>),
                None,
                Some(controlled_exit::<C>),
                None,
                None,
                None,
                ptr as _,
            );
        });
        unsafe {
            let boxptr = Box::from_raw(ptr);
            return NgSpice { callbacks: boxptr };
        }

        unsafe extern "C" fn send_char<C: Callbacks>(
            arg1: *mut c_char,
            _arg2: c_int,
            context: *mut c_void,
        ) -> c_int {
            let context = &mut *(context as *mut C);
            let str_res = CStr::from_ptr(arg1).to_str();
            if let Ok(s) = str_res {
                context.send_char(s);
            }
            0
        }
        unsafe extern "C" fn controlled_exit<C: Callbacks>(
            status: c_int,
            unload: bool,
            quit: bool,
            _instance: c_int,
            context: *mut c_void,
        ) -> c_int {
            let context = &mut *(context as *mut C);
            //TODO panic on use after exit
            context.controlled_exit(status as i32, unload, quit);
            0
        }
    }

    fn command(&self, s: &str) {
        let cs_res = CString::new(s);
        if let Ok(cs) = cs_res {
            let raw = cs.into_raw();
            unsafe {
                ngSpice_Command(raw);
                let _cs = CString::from_raw(raw);
            }
        }
    }
}

pub trait Callbacks {
    fn send_char(&mut self, _s: &str) {}
    fn controlled_exit(&mut self, _status: i32, _unload: bool, _quit: bool) {}
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Cb {
        strs: Vec<String>,
    }

    impl Callbacks for Cb {
        fn send_char(&mut self, s: &str) {
            self.strs.push(s.to_string())
        }
    }
    #[test]
    fn it_works() {
        let c = Cb { strs: Vec::new() };
        let spice = NgSpice::new(c);
        spice.command("echo hello");
        assert_eq!(
            spice.callbacks.strs.last().unwrap_or(&String::new()),
            "stdout hello"
        )
    }
}

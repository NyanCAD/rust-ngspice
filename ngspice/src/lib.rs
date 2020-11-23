use ngspice_sys::*;
use std::ffi::{CStr, CString};
use std::os::raw::{c_char, c_int, c_void};
use std::sync::Once;

#[derive(Debug)]
pub enum NgSpiceError {
    DoubleInitError,
}

static START: Once = Once::new();

pub struct NgSpice<C> {
    pub callbacks: C,
    exited: bool,
    initiated: bool,
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
        if self.initiated && !self.exited {
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
}

unsafe extern "C" fn send_char<C: Callbacks>(
    arg1: *mut c_char,
    _arg2: c_int,
    context: *mut c_void,
) -> c_int {
    let spice = &mut *(context as *mut NgSpice<C>);
    let cb = &mut spice.callbacks;
    let str_res = CStr::from_ptr(arg1).to_str();
    if let Ok(s) = str_res {
        cb.send_char(s);
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
    let spice = &mut *(context as *mut NgSpice<C>);
    let cb = &mut spice.callbacks;
    spice.exited = true;
    cb.controlled_exit(status as i32, unload, quit);
    0
}

impl<C: Callbacks> NgSpice<C> {
    pub fn new(c: C) -> Result<Box<NgSpice<C>>, NgSpiceError> {
        let spice = NgSpice {
                    callbacks: c,
                    exited: false,
                    initiated: false,
                };
        let ptr = Box::new(spice);
        let rawptr = Box::into_raw(ptr);
        START.call_once(|| unsafe {
            ngSpice_Init(
                Some(send_char::<C>),
                None,
                Some(controlled_exit::<C>),
                None,
                None,
                None,
                rawptr as _,
            );
            (*rawptr).initiated = true;
        });
        unsafe {
            let ptr = Box::from_raw(rawptr);
            if ptr.initiated {
                return Ok(ptr);
            } else {
                return Err(NgSpiceError::DoubleInitError);
            }
        }

    }

    pub fn command(&self, s: &str) {
        if self.exited {
            panic!("NgSpice exited")
        }
        let cs_res = CString::new(s);
        if let Ok(cs) = cs_res {
            let raw = cs.into_raw();
            unsafe {
                let ret = ngSpice_Command(raw);
                println!("{}", ret);
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
            print!("{}\n", s);
            self.strs.push(s.to_string())
        }
    }
    #[test]
    fn it_works() {
        let c = Cb { strs: Vec::new() };
        let spice = NgSpice::new(c).unwrap();
        assert!(NgSpice::new(Cb { strs: Vec::new() }).is_err());
        spice.command("echo hello");
        assert_eq!(
            spice.callbacks.strs.last().unwrap_or(&String::new()),
            "stdout hello"
        );
        spice.command("exit");
        let result = std::panic::catch_unwind(|| spice.command("echo hello"));
        assert!(result.is_err());
    }
}

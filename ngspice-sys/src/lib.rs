#![allow(non_upper_case_globals)]
#![allow(non_camel_case_types)]
include!(concat!(env!("OUT_DIR"), "/bindings.rs"));

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::{CString, CStr, c_void};
    use std::os::raw::{c_char, c_int};

    extern "C" fn controlled_exit(_arg1: c_int, _arg2: bool, _arg3: bool, _arg4: c_int, _arg5: *mut c_void) -> c_int {
        return 0;
    }

    static mut responses: Vec<String> = Vec::new();

    unsafe extern "C" fn send_char(arg1: *mut c_char, _arg2: c_int, _arg3: *mut c_void) -> c_int {
        let s = CStr::from_ptr(arg1).to_str().expect("could not make string");
        print!("{}", s);
        responses.push(s.to_string());
        return 0;
    }

    #[test]
    fn init_works() {
        unsafe {
            ngSpice_Init(Some(send_char), None, Some(controlled_exit), None, None, None, std::ptr::null_mut());
            let s = CString::new("echo hello").expect("CString::new failed");
            ngSpice_Command(s.into_raw());
            assert_eq!(responses.last().unwrap_or(&String::new()), "stdout hello");
        }
    }
}

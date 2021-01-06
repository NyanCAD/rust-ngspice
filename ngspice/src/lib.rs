use ngspice_sys::*;
use std::ffi::{CStr, CString, NulError};
use std::os::raw::{c_char, c_int, c_void};
use std::sync::Once;
use std::convert::TryInto;

#[derive(Debug)]
pub enum NgSpiceError {
    DoubleInitError,
    CommandError,
    EncodingError,
}

static START: Once = Once::new();

#[derive(Debug)]
pub struct NgSpice<C> {
    pub callbacks: C,
    exited: bool,
    initiated: bool,
}

#[derive(Debug)]
pub struct VectorInfo {
    pub name: String,
    pub data: Vec<f64>,
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

impl From<NulError> for NgSpiceError {
    fn from(_e: NulError) -> NgSpiceError {
        NgSpiceError::EncodingError
    }
}

impl From<std::str::Utf8Error> for NgSpiceError {
    fn from(_e: std::str::Utf8Error) -> NgSpiceError {
        NgSpiceError::EncodingError
    }
}

impl From<std::num::TryFromIntError> for NgSpiceError {
    fn from(_e: std::num::TryFromIntError) -> NgSpiceError {
        NgSpiceError::EncodingError
    }
}

impl<C: Callbacks> NgSpice<C> {
    pub fn new(c: C) -> Result<std::sync::Arc<NgSpice<C>>, NgSpiceError> {
        let spice = NgSpice {
            callbacks: c,
            exited: false,
            initiated: false,
        };
        let mut ptr = std::sync::Arc::new(spice);
        let rawptr = std::sync::Arc::as_ptr(&ptr);
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
            std::sync::Arc::get_mut(&mut ptr).unwrap().initiated = true;
        });
        if ptr.initiated {
            return Ok(ptr);
        } else {
            return Err(NgSpiceError::DoubleInitError);
        }
    }

    pub fn command(&self, s: &str) -> Result<(), NgSpiceError> {
        if self.exited {
            panic!("NgSpice exited")
        }
        let cs_res = CString::new(s);
        if let Ok(cs) = cs_res {
            let raw = cs.into_raw();
            unsafe {
                let ret = ngSpice_Command(raw);
                CString::from_raw(raw);
                if ret == 0 {
                    Ok(())
                } else {
                    Err(NgSpiceError::CommandError)
                }
            }
        } else {
            Err(NgSpiceError::EncodingError)
        }
    }

    pub fn circuit(&self, circ: &[&str]) -> Result<(), NgSpiceError> {
        let buf_res: Result<Vec<*mut i8>, _> = circ
            .iter()
            .map(|s| CString::new(*s).map(|cs| cs.into_raw()))
            .collect();
        if let Ok(mut buf) = buf_res {
            // ngspice wants an empty string and a nullptr
            buf.push(CString::new("").unwrap().into_raw());
            buf.push(std::ptr::null_mut());
            unsafe {
                let res = ngSpice_Circ(buf.as_mut_ptr());
                for b in buf {
                    if !b.is_null() {
                        CString::from_raw(b); // drop strings
                    }
                }
                if res == 1 {
                    Err(NgSpiceError::CommandError)
                } else {
                    Ok(())
                }
            }
        } else {
            Err(NgSpiceError::EncodingError)
        }
    }

    pub fn current_plot(&self) -> Result<String, NgSpiceError> {
        unsafe {
            let ret = ngSpice_CurPlot();
            let ptr_res = CStr::from_ptr(ret).to_str();
            if let Ok(ptr) = ptr_res {
                Ok(String::from(ptr))
            } else {
                return Err(NgSpiceError::EncodingError);
            }
        }
    }

    pub fn all_plots(&self) -> Result<Vec<String>, NgSpiceError> {
        unsafe {
            let ptrs = ngSpice_AllPlots();
            let mut strs: Vec<String> = Vec::new();
            let mut i = 0;
            while !(*ptrs.offset(i)).is_null() {
                let ptr_res = CStr::from_ptr(*ptrs.offset(i)).to_str();
                if let Ok(ptr) = ptr_res {
                    let s = String::from(ptr);
                    strs.push(s);
                } else {
                    return Err(NgSpiceError::EncodingError);
                }
                i+=1;
            }
            return Ok(strs);
        }
    }

    pub fn all_vecs(&self, plot: &str) -> Result<Vec<String>, NgSpiceError> {
        let cs_res = CString::new(plot);
        if let Ok(cs) = cs_res {
            let raw = cs.into_raw();
            unsafe {
                let ptrs = ngSpice_AllVecs(raw);
                CString::from_raw(raw);
                let mut strs: Vec<String> = Vec::new();
                let mut i = 0;
                while !(*ptrs.offset(i)).is_null() {
                    let ptr_res = CStr::from_ptr(*ptrs.offset(i)).to_str();
                    if let Ok(ptr) = ptr_res {
                        let s = String::from(ptr);
                        strs.push(s);
                    } else {
                        return Err(NgSpiceError::EncodingError);
                    }
                    i+=1;
                }
                Ok(strs)
            }
        } else {
            Err(NgSpiceError::EncodingError)
        }
    }

    pub fn vector_info(&self, vec: &str) -> Result<VectorInfo, NgSpiceError> {
        let cs = CString::new(vec)?;
        let raw = cs.into_raw();
        unsafe {
            let vecinfo = *ngGet_Vec_Info(raw);
            CString::from_raw(raw);
            let ptr = CStr::from_ptr(vecinfo.v_name).to_str()?;
            let len = vecinfo.v_length.try_into()?;
            let s = String::from(ptr);
            if !vecinfo.v_realdata.is_null() {
                let real_slice = std::slice::from_raw_parts_mut(vecinfo.v_realdata, len);
                return Ok(VectorInfo {
                    name: s,
                    data: Vec::from(real_slice),
                })
            } else { // todo complex data
                return Ok(VectorInfo {
                    name: s,
                    data: Vec::new(),
                })
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
        spice.command("echo hello").expect("echo failed");
        assert_eq!(
            spice.callbacks.strs.last().unwrap_or(&String::new()),
            "stdout hello"
        );
        spice.circuit(&[
                ".title KiCad schematic",
                ".MODEL FAKE_NMOS NMOS (LEVEL=3 VTO=0.75)",
                ".save all @m1[gm] @m1[id] @m1[vgs] @m1[vds] @m1[vto]",
                "R1 /vdd /drain 10k",
                "M1 /drain /gate GND GND FAKE_NMOS W=10u L=1u",
                "V1 /vdd GND dc(5)",
                "V2 /gate GND dc(2)",
                ".end",
            ])
            .expect("circuit failed");
        spice.command("op").expect("op failed");
        spice.command("alter m1 W=20u").expect("op failed");
        spice.command("op").expect("op failed");
        let plots = spice.all_plots().expect("plots failed");
        println!("{:?}", plots);
        assert_eq!(plots[0], "op2");
        let curplot = spice.current_plot().expect("curplot failed");
        assert_eq!(curplot, "op2");
        for plot in plots {
            let vecs = spice.all_vecs(&plot).expect("vecs");
            println!("{}: {:?}", plot, vecs);
            for vec in vecs {
                let vecinfo = spice.vector_info(&format!("{}.{}", plot, vec));
                println!("{:?}", vecinfo);
            }
        }
        //spice.command("quit").expect("quit failed");
        //let result = std::panic::catch_unwind(|| spice.command("echo hello"));
        //assert!(result.is_err());
    }
}

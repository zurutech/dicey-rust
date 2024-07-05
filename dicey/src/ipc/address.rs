use std::{
    ffi::{CStr, CString},
    mem, path,
};

use dicey_sys::{dicey_addr, dicey_addr_deinit, dicey_addr_from_str};

pub struct Address {
    caddr: dicey_addr,
}

impl Address {
    pub fn new(addr: impl AsRef<[u8]>) -> Self {
        CString::new(addr.as_ref())
            .expect("malformed null values in string")
            .as_ref()
            .into()
    }

    pub(crate) fn into_raw(self) -> dicey_addr {
        let caddr = self.caddr;

        mem::forget(self);

        caddr
    }
}

impl Drop for Address {
    fn drop(&mut self) {
        unsafe {
            dicey_addr_deinit(&mut self.caddr);
        }
    }
}

impl From<&path::Path> for Address {
    fn from(path: &path::Path) -> Self {
        Self::new(path.as_os_str().as_encoded_bytes())
    }
}

impl From<&str> for Address {
    fn from(addr: &str) -> Self {
        Self::new(addr.as_bytes())
    }
}

impl From<&String> for Address {
    fn from(addr: &String) -> Self {
        (addr as &str).into()
    }
}

impl From<&CStr> for Address {
    fn from(addr: &CStr) -> Self {
        let caddr = unsafe {
            let mut caddr = mem::zeroed();

            // Rust generally doesn't handle memory failures, so aborting isn't that bad
            assert!(!dicey_addr_from_str(&mut caddr, addr.as_ptr()).is_null());

            caddr
        };

        Self { caddr }
    }
}

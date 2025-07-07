/*
 * Copyright (c) 2014-2024 Zuru Tech HK Limited, All rights reserved.
 *
 * Licensed under the Apache License, Version 2.0 (the "License");
 * you may not use this file except in compliance with the License.
 * You may obtain a copy of the License at
 *
 *     http://www.apache.org/licenses/LICENSE-2.0
 *
 * Unless required by applicable law or agreed to in writing, software
 * distributed under the License is distributed on an "AS IS" BASIS,
 * WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
 * See the License for the specific language governing permissions and
 * limitations under the License.
 */

// Copyright (c) 2014-2024 Zuru Tech HK Limited, All rights reserved.

use std::{
    env,
    ffi::CStr,
    fs::File,
    io::{self, Write},
    path::PathBuf,
    ptr, slice,
};

use dicey_sys::*;

unsafe fn gen_errors(mut write: impl Write) -> io::Result<()> {
    let mut defs_ptr: *const dicey_error_def = ptr::null();
    let mut count = 0usize;

    unsafe {
        dicey_error_infos(&mut defs_ptr, &mut count);
    }

    writeln!(
        write,
        r#"use std::{{error, ffi::CStr, fmt::Display}};
        
use dicey_sys::{{dicey_error, dicey_error_msg}};
"#
    )?;

    writeln!(
        write,
        "#[derive(Clone, Copy, Debug, Eq, PartialEq)]\n#[repr(i32)]\npub enum Error {{"
    )?;

    let defs = unsafe { slice::from_raw_parts(defs_ptr, count) };

    for def in defs {
        let name = unsafe { CStr::from_ptr(def.name) }.to_str().unwrap(); // we assume all strings are ASCII
        writeln!(write, "    {} = {},", name, def.errnum)?;
    }

    writeln!(write, "}}\n")?;

    writeln!(
        write,
        r#"impl Error {{
    pub const fn code(self) -> i32 {{
        self as i32
    }}
}}

impl error::Error for Error {{}}
"#
    )?;

    writeln!(
        write,
        r#"impl Display for Error {{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {{
        let msg = unsafe {{
            let cmsg = dicey_error_msg(self.code());
            CStr::from_ptr(cmsg).to_str().unwrap()
        }};

        write!(f, "{{msg}}")
    }}
}}

impl From<dicey_error> for Error {{
    fn from(err: dicey_error) -> Self {{
        match err {{"#
    )?;

    for def in defs {
        let name = unsafe { CStr::from_ptr(def.name) }.to_str().unwrap(); // we assume all strings are ASCII

        writeln!(write, "           {} => Error::{},", def.errnum, name)?;
    }

    writeln!(
        write,
        r#"            _ => unreachable!(), // C library mismatch or bug
        }}
    }}
}}
"#
    )
}

fn main() {
    let out_path = PathBuf::from(env::var("OUT_DIR").unwrap());

    let mut file = File::create(out_path.join("errors.rs")).unwrap();

    unsafe {
        gen_errors(&mut file).unwrap();
    }
}

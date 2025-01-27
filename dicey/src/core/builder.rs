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

use std::{
    borrow::Cow,
    ffi::{c_char, CStr, CString},
    mem,
};

use uuid::Uuid;

use dicey_sys::{
    dicey_arg, dicey_arg__bindgen_ty_1, dicey_bytes_arg, dicey_error_arg, dicey_message_builder,
    dicey_message_builder_begin, dicey_message_builder_build, dicey_message_builder_discard,
    dicey_message_builder_init, dicey_message_builder_set_path, dicey_message_builder_set_selector,
    dicey_message_builder_value_end, dicey_message_builder_value_start, dicey_selector, dicey_uuid,
    dicey_value_builder, dicey_value_builder_array_end, dicey_value_builder_array_start,
    dicey_value_builder_next, dicey_value_builder_pair_end, dicey_value_builder_pair_start,
    dicey_value_builder_set, dicey_value_builder_tuple_end, dicey_value_builder_tuple_start,
};

use super::{
    macros::ccall,
    value::{bytes_to_cpath, Byte, ErrorMessage, Path, Type},
    Error, Message, Op, RawPacket, Selector,
};

pub struct MessageBuilder {
    cbuilder: dicey_message_builder,

    seq: Option<u32>,
    path: Option<CString>,
    tname: Option<CString>,
    elem: Option<CString>,

    cache: Vec<Vec<u8>>,
}

impl MessageBuilder {
    pub fn event() -> Result<Self, Error> {
        Self::new(Op::Event)
    }

    pub fn exec() -> Result<Self, Error> {
        Self::new(Op::Exec)
    }

    pub fn get() -> Result<Self, Error> {
        Self::new(Op::Get)
    }

    pub fn new(kind: Op) -> Result<Self, Error> {
        let cbuilder = unsafe {
            let mut cbuilder = mem::zeroed();

            ccall!(message_builder_init, &mut cbuilder)?;
            ccall!(message_builder_begin, &mut cbuilder, kind.as_c())?;

            cbuilder
        };

        Ok(Self {
            cbuilder,

            seq: None,
            path: None,
            tname: None,
            elem: None,

            cache: Vec::new(),
        })
    }

    pub fn response() -> Result<Self, Error> {
        Self::new(Op::Response)
    }

    pub fn set() -> Result<Self, Error> {
        Self::new(Op::Set)
    }

    pub fn build(mut self) -> Result<Message, Error> {
        let mut cmsg = unsafe { mem::zeroed() };

        unsafe {
            ccall!(message_builder_build, &mut self.cbuilder, &mut cmsg)?;
        }

        RawPacket::from(cmsg).try_into()
    }

    pub fn path(mut self, path: impl Into<Vec<u8>>) -> Result<Self, Error> {
        let cstr = bytes_to_cpath(path)?;

        unsafe {
            ccall!(message_builder_set_path, &mut self.cbuilder, cstr.as_ptr())?;
        }

        self.path = Some(cstr);

        Ok(self)
    }

    pub fn selector<'a>(mut self, sel: impl Into<Selector<'a>>) -> Result<Self, Error> {
        let sel = sel.into();

        let trait_name = strip_null(sel.trait_name);
        let elem = strip_null(sel.elem);

        let tname = Some(CString::new(trait_name).map_err(|_| Error::InvalidData)?);
        let elem = Some(CString::new(elem).map_err(|_| Error::InvalidData)?);

        let csel = dicey_selector {
            trait_: tname.as_ref().unwrap().as_ptr() as *const c_char,
            elem: elem.as_ref().unwrap().as_ptr() as *const c_char,
        };

        unsafe {
            ccall!(message_builder_set_selector, &mut self.cbuilder, csel)?;
        }

        self.tname = tname;
        self.elem = elem;

        Ok(self)
    }

    pub const fn seq(mut self, seq: u32) -> Self {
        self.seq = Some(seq);

        self
    }

    pub fn value(self, value: impl ToDicey) -> Result<Self, Error> {
        self.value_with(|builder| builder.set(value))
    }

    pub fn value_with<F>(mut self, op: F) -> Result<Self, Error>
    where
        F: FnOnce(&mut ValueBuilder) -> Result<(), Error>,
    {
        unsafe {
            // do not move the cbuilder! the message builder expects the value builder to be in the same memory location
            // for the duration of the value building process.
            let mut valbuilder = ValueBuilder {
                cache: &mut self.cache,
                cbuilder: mem::zeroed(),
            };

            ccall!(
                message_builder_value_start,
                &mut self.cbuilder,
                &mut valbuilder.cbuilder
            )?;

            let res = op(&mut valbuilder);

            let end_res = ccall!(
                message_builder_value_end,
                &mut self.cbuilder,
                &mut valbuilder.cbuilder
            );

            res.and_then(|_| end_res.map(|_| self))
        }
    }
}

impl Drop for MessageBuilder {
    fn drop(&mut self) {
        unsafe {
            dicey_message_builder_discard(&mut self.cbuilder);
        }
    }
}

pub struct ValueBuilder<'a> {
    cache: &'a mut Vec<Vec<u8>>,
    cbuilder: dicey_value_builder,
}

impl ValueBuilder<'_> {
    pub fn set(&mut self, value: impl ToDicey) -> Result<(), Error> {
        value.to_dicey(self)
    }
}

pub trait ToDicey {
    const TYPE_KIND: Type;

    fn to_dicey(&self, builder: &mut ValueBuilder) -> Result<(), Error>;
}

macro_rules! impl_to_dicey {
    ($ty:ident, $field: ident, $c_type: ident, $kind: ident) => {
        impl ToDicey for $ty {
            const TYPE_KIND: Type = Type::$kind;

            fn to_dicey(&self, builder: &mut ValueBuilder) -> Result<(), Error> {
                use paste::paste;

                let c_ty = paste!(dicey_sys::[<dicey_type_DICEY_TYPE_ $c_type>]);

                let arg = paste! {
                    dicey_arg {
                        type_: c_ty,
                        __bindgen_anon_1: dicey_arg__bindgen_ty_1 {
                            $field: From::from(*self),
                        }
                    }
                };

                unsafe {
                    ccall!(value_builder_set, &mut builder.cbuilder, arg)
                }.map(|_| ())
            }
        }
    };
}

macro_rules! impl_to_dicey_int {
    ($ty:ident, $c_type: ident, $kind: ident) => {
        paste::paste! {
            impl_to_dicey!($ty, [<$ty _>], $c_type, $kind);
        }
    };
}

impl ToDicey for () {
    const TYPE_KIND: Type = Type::Unit;

    fn to_dicey(&self, builder: &mut ValueBuilder) -> Result<(), Error> {
        unsafe {
            ccall!(
                value_builder_set,
                &mut builder.cbuilder,
                dicey_arg {
                    type_: dicey_sys::dicey_type_DICEY_TYPE_UNIT,
                    __bindgen_anon_1: mem::zeroed(), // no value needed, Dicey will not read from this field
                }
            )
        }
        .map(|_| ())
    }
}

impl ToDicey for bool {
    const TYPE_KIND: Type = Type::Bool;

    fn to_dicey(&self, builder: &mut ValueBuilder) -> Result<(), Error> {
        unsafe {
            ccall!(
                value_builder_set,
                &mut builder.cbuilder,
                dicey_arg {
                    type_: dicey_sys::dicey_type_DICEY_TYPE_BOOL,
                    __bindgen_anon_1: dicey_arg__bindgen_ty_1 {
                        boolean: *self as u8,
                    }
                }
            )
        }
        .map(|_| ())
    }
}

impl_to_dicey!(Byte, byte, BYTE, Byte);
impl_to_dicey!(f32, floating, FLOAT, Float);
impl_to_dicey!(f64, floating, FLOAT, Float);

impl_to_dicey_int!(i16, INT16, Int16);
impl_to_dicey_int!(i32, INT32, Int32);
impl_to_dicey_int!(i64, INT64, Int64);
impl_to_dicey_int!(u16, UINT16, UInt16);
impl_to_dicey_int!(u32, UINT32, UInt32);
impl_to_dicey_int!(u64, UINT64, UInt64);

impl ToDicey for Uuid {
    const TYPE_KIND: Type = Type::Uuid;

    fn to_dicey(&self, builder: &mut ValueBuilder) -> Result<(), Error> {
        unsafe {
            ccall!(
                value_builder_set,
                &mut builder.cbuilder,
                dicey_arg {
                    type_: dicey_sys::dicey_type_DICEY_TYPE_UUID,
                    __bindgen_anon_1: dicey_arg__bindgen_ty_1 {
                        uuid: dicey_uuid {
                            bytes: *self.as_bytes(),
                        },
                    }
                }
            )
            .map(|_| ())
        }
    }
}

impl<T: ToDicey> ToDicey for [T] {
    const TYPE_KIND: Type = Type::Array;

    fn to_dicey(&self, builder: &mut ValueBuilder) -> Result<(), Error> {
        unsafe {
            ccall!(
                value_builder_array_start,
                &mut builder.cbuilder,
                T::TYPE_KIND.to_c()
            )?;

            for value in self {
                let mut item = mem::zeroed();

                ccall!(value_builder_next, &mut builder.cbuilder, &mut item)?;

                let mut child = ValueBuilder {
                    cache: builder.cache,
                    cbuilder: item,
                };

                value.to_dicey(&mut child)?;
            }

            ccall!(value_builder_array_end, &mut builder.cbuilder)?;
        }

        Ok(())
    }
}

impl<T: ToDicey> ToDicey for Vec<T> {
    const TYPE_KIND: Type = <[T] as ToDicey>::TYPE_KIND;

    fn to_dicey(&self, builder: &mut ValueBuilder) -> Result<(), Error> {
        self.as_slice().to_dicey(builder)
    }
}

impl<T: ToDicey, U: ToDicey> ToDicey for (T, U) {
    const TYPE_KIND: Type = Type::Pair;

    fn to_dicey(&self, builder: &mut ValueBuilder) -> Result<(), Error> {
        unsafe {
            ccall!(value_builder_pair_start, &mut builder.cbuilder)?;

            let mut item = mem::zeroed();

            ccall!(value_builder_next, &mut builder.cbuilder, &mut item)?;

            let mut child = ValueBuilder {
                cache: builder.cache,
                cbuilder: item,
            };

            self.0.to_dicey(&mut child)?;

            ccall!(value_builder_next, &mut builder.cbuilder, &mut item)?;

            let mut child = ValueBuilder {
                cache: builder.cache,
                cbuilder: item,
            };

            self.1.to_dicey(&mut child)?;

            ccall!(value_builder_pair_end, &mut builder.cbuilder)?;
        }

        Ok(())
    }
}

macro_rules! impl_to_dicey_tuple {
    ($($name:ident)+) => {
        impl<$($name: ToDicey),+> ToDicey for ($($name,)+) {
            const TYPE_KIND: Type = Type::Tuple;

            fn to_dicey(&self, builder: &mut ValueBuilder) -> Result<(), Error> {
                // we need this, otherwise the hack below won't work
                #![allow(non_snake_case)]

                let ($($name,)+) = self;

                unsafe {
                    ccall!(
                        value_builder_tuple_start,
                        &mut builder.cbuilder
                    )?;

                    let mut item = mem::zeroed();

                    $({
                        ccall!(value_builder_next, &mut builder.cbuilder, &mut item)?;

                        let mut child = ValueBuilder {
                            cache: builder.cache,
                            cbuilder: item,
                        };

                        $name.to_dicey(&mut child)?;
                    })+;

                    ccall!(value_builder_tuple_end, &mut builder.cbuilder)?;
                }

                Ok(())
            }
        }
    };
}

// implement ToDicey for tuples of size 1 to 32
impl_to_dicey_tuple!(A B C);
impl_to_dicey_tuple!(A B C D);
impl_to_dicey_tuple!(A B C D E);
impl_to_dicey_tuple!(A B C D E F);
impl_to_dicey_tuple!(A B C D E F G);
impl_to_dicey_tuple!(A B C D E F G H);
impl_to_dicey_tuple!(A B C D E F G H I);
impl_to_dicey_tuple!(A B C D E F G H I J);
impl_to_dicey_tuple!(A B C D E F G H I J K);
impl_to_dicey_tuple!(A B C D E F G H I J K L);
impl_to_dicey_tuple!(A B C D E F G H I J K L M);
impl_to_dicey_tuple!(A B C D E F G H I J K L M N);
impl_to_dicey_tuple!(A B C D E F G H I J K L M N O);
impl_to_dicey_tuple!(A B C D E F G H I J K L M N O P);
impl_to_dicey_tuple!(A B C D E F G H I J K L M N O P Q);
impl_to_dicey_tuple!(A B C D E F G H I J K L M N O P Q R);
impl_to_dicey_tuple!(A B C D E F G H I J K L M N O P Q R S);
impl_to_dicey_tuple!(A B C D E F G H I J K L M N O P Q R S T);
impl_to_dicey_tuple!(A B C D E F G H I J K L M N O P Q R S T U);
impl_to_dicey_tuple!(A B C D E F G H I J K L M N O P Q R S T U V);
impl_to_dicey_tuple!(A B C D E F G H I J K L M N O P Q R S T U V W);
impl_to_dicey_tuple!(A B C D E F G H I J K L M N O P Q R S T U V W X);
impl_to_dicey_tuple!(A B C D E F G H I J K L M N O P Q R S T U V W X Y);
impl_to_dicey_tuple!(A B C D E F G H I J K L M N O P Q R S T U V W X Y Z);
impl_to_dicey_tuple!(A B C D E F G H I J K L M N O P Q R S T U V W X Y Z あ);
impl_to_dicey_tuple!(A B C D E F G H I J K L M N O P Q R S T U V W X Y Z あ い);
impl_to_dicey_tuple!(A B C D E F G H I J K L M N O P Q R S T U V W X Y Z あ い う);
impl_to_dicey_tuple!(A B C D E F G H I J K L M N O P Q R S T U V W X Y Z あ い う え);
impl_to_dicey_tuple!(A B C D E F G H I J K L M N O P Q R S T U V W X Y Z あ い う え お);
impl_to_dicey_tuple!(A B C D E F G H I J K L M N O P Q R S T U V W X Y Z あ い う え お か);

impl ToDicey for [u8] {
    const TYPE_KIND: Type = Type::Bytes;

    fn to_dicey(&self, builder: &mut ValueBuilder) -> Result<(), Error> {
        builder.cache.push(self.to_owned());

        let stored_payload = builder.cache.last().unwrap();

        unsafe {
            ccall!(
                value_builder_set,
                &mut builder.cbuilder,
                dicey_arg {
                    type_: dicey_sys::dicey_type_DICEY_TYPE_BYTES,
                    __bindgen_anon_1: dicey_arg__bindgen_ty_1 {
                        bytes: dicey_bytes_arg {
                            len: stored_payload
                                .len()
                                .try_into()
                                .map_err(|_| Error::Overflow)?,
                            data: stored_payload.as_ptr(),
                        },
                    }
                }
            )
        }
        .map(|_| ())
    }
}

impl ToDicey for &'_ [u8] {
    const TYPE_KIND: Type = <[u8] as ToDicey>::TYPE_KIND;

    fn to_dicey(&self, builder: &mut ValueBuilder) -> Result<(), Error> {
        <[u8]>::to_dicey(*self, builder)
    }
}

impl ToDicey for Cow<'_, [u8]> {
    const TYPE_KIND: Type = <str as ToDicey>::TYPE_KIND;

    fn to_dicey(&self, builder: &mut ValueBuilder) -> Result<(), Error> {
        self.as_ref().to_dicey(builder)
    }
}

impl ToDicey for Vec<u8> {
    const TYPE_KIND: Type = <[u8] as ToDicey>::TYPE_KIND;

    fn to_dicey(&self, builder: &mut ValueBuilder) -> Result<(), Error> {
        self.as_slice().to_dicey(builder)
    }
}

impl ToDicey for &'_ Vec<u8> {
    const TYPE_KIND: Type = <[u8] as ToDicey>::TYPE_KIND;

    fn to_dicey(&self, builder: &mut ValueBuilder) -> Result<(), Error> {
        Vec::to_dicey(*self, builder)
    }
}

impl ToDicey for str {
    const TYPE_KIND: Type = Type::String;

    fn to_dicey(&self, builder: &mut ValueBuilder) -> Result<(), Error> {
        builder.cache.push(
            CString::new(self)
                .map_err(|_| Error::InvalidData)?
                .into_bytes_with_nul(),
        );

        unsafe {
            ccall!(
                value_builder_set,
                &mut builder.cbuilder,
                dicey_arg {
                    type_: dicey_sys::dicey_type_DICEY_TYPE_STR,
                    __bindgen_anon_1: dicey_arg__bindgen_ty_1 {
                        str_: builder.cache.last().unwrap().as_ptr() as *const c_char,
                    }
                }
            )
        }
        .map(|_| ())
    }
}

impl ToDicey for &'_ str {
    const TYPE_KIND: Type = <str as ToDicey>::TYPE_KIND;

    fn to_dicey(&self, builder: &mut ValueBuilder) -> Result<(), Error> {
        str::to_dicey(*self, builder)
    }
}

impl ToDicey for Cow<'_, str> {
    const TYPE_KIND: Type = <str as ToDicey>::TYPE_KIND;

    fn to_dicey(&self, builder: &mut ValueBuilder) -> Result<(), Error> {
        self.as_ref().to_dicey(builder)
    }
}

impl ToDicey for String {
    const TYPE_KIND: Type = <CStr as ToDicey>::TYPE_KIND;

    fn to_dicey(&self, builder: &mut ValueBuilder) -> Result<(), Error> {
        self.as_str().to_dicey(builder)
    }
}

impl ToDicey for &'_ String {
    const TYPE_KIND: Type = <String as ToDicey>::TYPE_KIND;

    fn to_dicey(&self, builder: &mut ValueBuilder) -> Result<(), Error> {
        String::to_dicey(*self, builder)
    }
}

impl ToDicey for CStr {
    const TYPE_KIND: Type = Type::String;

    fn to_dicey(&self, builder: &mut ValueBuilder) -> Result<(), Error> {
        builder.cache.push(self.to_bytes_with_nul().to_owned());

        unsafe {
            ccall!(
                value_builder_set,
                &mut builder.cbuilder,
                dicey_arg {
                    type_: dicey_sys::dicey_type_DICEY_TYPE_STR,
                    __bindgen_anon_1: dicey_arg__bindgen_ty_1 {
                        str_: builder.cache.last().unwrap().as_ptr() as *const c_char,
                    }
                }
            )
        }
        .map(|_| ())
    }
}

impl ToDicey for CString {
    const TYPE_KIND: Type = <CStr as ToDicey>::TYPE_KIND;

    fn to_dicey(&self, builder: &mut ValueBuilder) -> Result<(), Error> {
        self.as_c_str().to_dicey(builder)
    }
}

impl ToDicey for Path<'_> {
    const TYPE_KIND: Type = Type::Path;

    fn to_dicey(&self, builder: &mut ValueBuilder) -> Result<(), Error> {
        builder.cache.push(
            CString::new(self as &str)
                .map_err(|_| Error::InvalidData)?
                .into_bytes_with_nul(),
        );

        unsafe {
            ccall!(
                value_builder_set,
                &mut builder.cbuilder,
                dicey_arg {
                    type_: dicey_sys::dicey_type_DICEY_TYPE_PATH,
                    __bindgen_anon_1: dicey_arg__bindgen_ty_1 {
                        str_: builder.cache.last().unwrap().as_ptr() as *const c_char,
                    }
                }
            )
        }
        .map(|_| ())
    }
}

impl ToDicey for Selector<'_> {
    const TYPE_KIND: Type = Type::Selector;

    fn to_dicey(&self, builder: &mut ValueBuilder) -> Result<(), Error> {
        builder.cache.push(
            CString::new(self.trait_name)
                .map_err(|_| Error::InvalidData)?
                .into_bytes_with_nul(),
        );

        builder.cache.push(
            CString::new(self.elem)
                .map_err(|_| Error::InvalidData)?
                .into_bytes_with_nul(),
        );

        let (trait_name, elem) = match &builder.cache[builder.cache.len() - 2..] {
            [tname, elem] => (
                tname.as_ptr() as *const c_char,
                elem.as_ptr() as *const c_char,
            ),
            _ => unreachable!(),
        };

        let arg = dicey_arg {
            type_: dicey_sys::dicey_type_DICEY_TYPE_SELECTOR,
            __bindgen_anon_1: dicey_arg__bindgen_ty_1 {
                selector: dicey_selector {
                    trait_: trait_name,
                    elem,
                },
            },
        };

        unsafe { ccall!(value_builder_set, &mut builder.cbuilder, arg) }.map(|_| ())
    }
}

impl ToDicey for ErrorMessage<'_> {
    const TYPE_KIND: Type = Type::Error;

    fn to_dicey(&self, builder: &mut ValueBuilder) -> Result<(), Error> {
        let message = if self.message.is_some() {
            builder.cache.push(
                CString::new(self.message.unwrap())
                    .map_err(|_| Error::InvalidData)?
                    .into_bytes_with_nul(),
            );

            builder.cache.last().unwrap().as_ptr() as *const c_char
        } else {
            std::ptr::null()
        };

        unsafe {
            ccall!(
                value_builder_set,
                &mut builder.cbuilder,
                dicey_arg {
                    type_: dicey_sys::dicey_type_DICEY_TYPE_ERROR,
                    __bindgen_anon_1: dicey_arg__bindgen_ty_1 {
                        error: dicey_error_arg {
                            code: self.code,
                            message,
                        },
                    }
                }
            )
        }
        .map(|_| ())
    }
}

fn strip_null(bytes: &[u8]) -> &[u8] {
    if bytes.ends_with(&[0]) {
        &bytes[..bytes.len() - 1]
    } else {
        bytes
    }
}

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
    ffi::{CStr, CString},
    mem,
    ops::Deref,
    ptr, slice,
};

use dicey_sys::{
    dicey_errmsg, dicey_iterator_has_next, dicey_iterator_next, dicey_list, dicey_list_iter,
    dicey_list_type, dicey_pair, dicey_selector, dicey_type, dicey_type_DICEY_TYPE_ARRAY,
    dicey_type_DICEY_TYPE_BOOL, dicey_type_DICEY_TYPE_BYTE, dicey_type_DICEY_TYPE_BYTES,
    dicey_type_DICEY_TYPE_ERROR, dicey_type_DICEY_TYPE_FLOAT, dicey_type_DICEY_TYPE_INT16,
    dicey_type_DICEY_TYPE_INT32, dicey_type_DICEY_TYPE_INT64, dicey_type_DICEY_TYPE_PAIR,
    dicey_type_DICEY_TYPE_PATH, dicey_type_DICEY_TYPE_SELECTOR, dicey_type_DICEY_TYPE_STR,
    dicey_type_DICEY_TYPE_TUPLE, dicey_type_DICEY_TYPE_UINT16, dicey_type_DICEY_TYPE_UINT32,
    dicey_type_DICEY_TYPE_UINT64, dicey_type_DICEY_TYPE_UNIT, dicey_type_DICEY_TYPE_UUID,
    dicey_value, dicey_value_get_array, dicey_value_get_bool, dicey_value_get_byte,
    dicey_value_get_bytes, dicey_value_get_error, dicey_value_get_float, dicey_value_get_i16,
    dicey_value_get_i32, dicey_value_get_i64, dicey_value_get_pair, dicey_value_get_path,
    dicey_value_get_selector, dicey_value_get_str, dicey_value_get_tuple, dicey_value_get_type,
    dicey_value_get_u16, dicey_value_get_u32, dicey_value_get_u64, dicey_value_get_uuid,
};

use uuid::Uuid;

use super::{errors::Error, macros::ccall};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Type {
    Unit,
    Bool,
    Byte,
    Float,
    Int16,
    Int32,
    Int64,
    UInt16,
    UInt32,
    UInt64,
    Array,
    Tuple,
    Pair,
    Bytes,
    String,
    Uuid,
    Path,
    Selector,
    Error,
}

impl Type {
    pub(crate) const fn to_c(self) -> dicey_type {
        match self {
            Type::Unit => dicey_type_DICEY_TYPE_UNIT,
            Type::Bool => dicey_type_DICEY_TYPE_BOOL,
            Type::Byte => dicey_type_DICEY_TYPE_BYTE,
            Type::Float => dicey_type_DICEY_TYPE_FLOAT,
            Type::Int16 => dicey_type_DICEY_TYPE_INT16,
            Type::Int32 => dicey_type_DICEY_TYPE_INT32,
            Type::Int64 => dicey_type_DICEY_TYPE_INT64,
            Type::UInt16 => dicey_type_DICEY_TYPE_UINT16,
            Type::UInt32 => dicey_type_DICEY_TYPE_UINT32,
            Type::UInt64 => dicey_type_DICEY_TYPE_UINT64,
            Type::Array => dicey_type_DICEY_TYPE_ARRAY,
            Type::Tuple => dicey_type_DICEY_TYPE_TUPLE,
            Type::Pair => dicey_type_DICEY_TYPE_PAIR,
            Type::Bytes => dicey_type_DICEY_TYPE_BYTES,
            Type::String => dicey_type_DICEY_TYPE_STR,
            Type::Uuid => dicey_type_DICEY_TYPE_UUID,
            Type::Path => dicey_type_DICEY_TYPE_PATH,
            Type::Selector => dicey_type_DICEY_TYPE_SELECTOR,
            Type::Error => dicey_type_DICEY_TYPE_ERROR,
        }
    }
}

impl TryFrom<dicey_type> for Type {
    type Error = Error;

    fn try_from(value: dicey_type) -> Result<Self, Error> {
        match value {
            dicey_type_DICEY_TYPE_UNIT => Ok(Type::Unit),
            dicey_type_DICEY_TYPE_BOOL => Ok(Type::Bool),
            dicey_type_DICEY_TYPE_BYTE => Ok(Type::Byte),
            dicey_type_DICEY_TYPE_FLOAT => Ok(Type::Float),
            dicey_type_DICEY_TYPE_INT16 => Ok(Type::Int16),
            dicey_type_DICEY_TYPE_INT32 => Ok(Type::Int32),
            dicey_type_DICEY_TYPE_INT64 => Ok(Type::Int64),
            dicey_type_DICEY_TYPE_UINT16 => Ok(Type::UInt16),
            dicey_type_DICEY_TYPE_UINT32 => Ok(Type::UInt32),
            dicey_type_DICEY_TYPE_UINT64 => Ok(Type::UInt64),
            dicey_type_DICEY_TYPE_ARRAY => Ok(Type::Array),
            dicey_type_DICEY_TYPE_TUPLE => Ok(Type::Tuple),
            dicey_type_DICEY_TYPE_PAIR => Ok(Type::Pair),
            dicey_type_DICEY_TYPE_BYTES => Ok(Type::Bytes),
            dicey_type_DICEY_TYPE_STR => Ok(Type::String),
            dicey_type_DICEY_TYPE_UUID => Ok(Type::Uuid),
            dicey_type_DICEY_TYPE_PATH => Ok(Type::Path),
            dicey_type_DICEY_TYPE_SELECTOR => Ok(Type::Selector),
            dicey_type_DICEY_TYPE_ERROR => Ok(Type::Error),
            _ => Err(Error::InvalidData),
        }
    }
}

#[derive(Clone, Debug)]
pub enum ValueView<'a> {
    Unit,

    Bool(bool),
    Byte(Byte),
    Float(f64),

    Int16(i16),
    Int32(i32),
    Int64(i64),

    UInt16(u16),
    UInt32(u32),
    UInt64(u64),

    Array {
        element_kind: Type,
        items: Vec<ValueView<'a>>,
    },

    Tuple(Vec<ValueView<'a>>),
    Pair(Box<ValueView<'a>>, Box<ValueView<'a>>),

    Bytes(&'a [u8]),
    String(&'a str),

    Uuid(Uuid),

    Path(Path<'a>),
    Selector(Selector<'a>),

    Error(ErrorMessage<'a>),
}

impl<'a> ValueView<'a> {
    pub fn extract<T: FromDicey<'a>>(&self) -> Result<T, Error> {
        T::from_dicey(self)
    }

    pub const fn kind(&self) -> Type {
        match self {
            ValueView::Unit => Type::Unit,
            ValueView::Bool(_) => Type::Bool,
            ValueView::Byte(_) => Type::Byte,
            ValueView::Float(_) => Type::Float,
            ValueView::Int16(_) => Type::Int16,
            ValueView::Int32(_) => Type::Int32,
            ValueView::Int64(_) => Type::Int64,
            ValueView::UInt16(_) => Type::UInt16,
            ValueView::UInt32(_) => Type::UInt32,
            ValueView::UInt64(_) => Type::UInt64,
            ValueView::Array { .. } => Type::Array,
            ValueView::Tuple(_) => Type::Tuple,
            ValueView::Pair(_, _) => Type::Pair,
            ValueView::Bytes(_) => Type::Bytes,
            ValueView::String(_) => Type::String,
            ValueView::Uuid(_) => Type::Uuid,
            ValueView::Path(_) => Type::Path,
            ValueView::Selector(_) => Type::Selector,
            ValueView::Error(_) => Type::Error,
        }
    }
}

impl<'a> FromDicey<'a> for ValueView<'a> {
    fn from_dicey(value: &ValueView<'a>) -> Result<Self, Error> {
        Ok(value.clone())
    }
}

impl TryFrom<dicey_value> for ValueView<'_> {
    type Error = Error;

    fn try_from(value: dicey_value) -> Result<Self, Error> {
        let ty = unsafe { dicey_value_get_type(&value) };

        unsafe {
            match ty {
                dicey_type_DICEY_TYPE_UNIT => Ok(ValueView::Unit),

                dicey_type_DICEY_TYPE_BOOL => {
                    let mut ret = false;
                    ccall!(value_get_bool, &value, &mut ret)?;

                    Ok(ValueView::Bool(ret))
                }

                dicey_type_DICEY_TYPE_BYTE => {
                    let mut ret = 0u8;

                    ccall!(value_get_byte, &value, &mut ret)?;

                    Ok(ValueView::Byte(ret.into()))
                }

                dicey_type_DICEY_TYPE_FLOAT => {
                    let mut ret = 0.0f64;

                    ccall!(value_get_float, &value, &mut ret)?;

                    Ok(ValueView::Float(ret))
                }

                dicey_type_DICEY_TYPE_INT16 => {
                    let mut ret = 0i16;

                    ccall!(value_get_i16, &value, &mut ret)?;

                    Ok(ValueView::Int16(ret))
                }

                dicey_type_DICEY_TYPE_INT32 => {
                    let mut ret = 0i32;

                    ccall!(value_get_i32, &value, &mut ret)?;

                    Ok(ValueView::Int32(ret))
                }

                dicey_type_DICEY_TYPE_INT64 => {
                    let mut ret = 0i64;

                    ccall!(value_get_i64, &value, &mut ret)?;

                    Ok(ValueView::Int64(ret))
                }

                dicey_type_DICEY_TYPE_UINT16 => {
                    let mut ret = 0u16;

                    ccall!(value_get_u16, &value, &mut ret)?;

                    Ok(ValueView::UInt16(ret))
                }

                dicey_type_DICEY_TYPE_UINT32 => {
                    let mut ret = 0u32;

                    ccall!(value_get_u32, &value, &mut ret)?;

                    Ok(ValueView::UInt32(ret))
                }

                dicey_type_DICEY_TYPE_UINT64 => {
                    let mut ret = 0u64;

                    ccall!(value_get_u64, &value, &mut ret)?;

                    Ok(ValueView::UInt64(ret))
                }

                dicey_type_DICEY_TYPE_ARRAY => {
                    let mut list: dicey_list = mem::zeroed();

                    ccall!(value_get_array, &value, &mut list)?;

                    let ckind: dicey_type = dicey_list_type(&list)
                        .try_into()
                        .map_err(|_| Error::InvalidData)?;

                    Ok(ValueView::Array {
                        element_kind: Type::try_from(ckind)?,
                        items: extract_list(list)?,
                    })
                }

                dicey_type_DICEY_TYPE_TUPLE => {
                    let mut ret: dicey_list = mem::zeroed();

                    ccall!(value_get_tuple, &value, &mut ret)?;

                    Ok(ValueView::Tuple(extract_list(ret)?))
                }

                dicey_type_DICEY_TYPE_PAIR => {
                    let mut pair: dicey_pair = mem::zeroed();

                    ccall!(value_get_pair, &value, &mut pair)?;

                    Ok(ValueView::Pair(
                        Box::new(ValueView::try_from(pair.first)?),
                        Box::new(ValueView::try_from(pair.second)?),
                    ))
                }

                dicey_type_DICEY_TYPE_BYTES => {
                    let mut bytes = ptr::null();
                    let mut nbytes = 0usize;

                    ccall!(value_get_bytes, &value, &mut bytes, &mut nbytes)?;

                    Ok(ValueView::Bytes(slice::from_raw_parts(bytes, nbytes)))
                }

                dicey_type_DICEY_TYPE_STR => {
                    let mut bytes = ptr::null();

                    ccall!(value_get_str, &value, &mut bytes)?;

                    CStr::from_ptr(bytes)
                        .to_str()
                        .map(ValueView::String)
                        .map_err(|_| Error::BadMessage)
                }

                dicey_type_DICEY_TYPE_UUID => {
                    let mut uuid = mem::zeroed();

                    ccall!(value_get_uuid, &value, &mut uuid)?;

                    Ok(ValueView::Uuid(Uuid::from_bytes(uuid.bytes)))
                }

                dicey_type_DICEY_TYPE_PATH => {
                    let mut bytes = ptr::null();

                    ccall!(value_get_path, &value, &mut bytes)?;

                    CStr::from_ptr(bytes)
                        .to_str()
                        .map(|s| ValueView::Path(Path(s)))
                        .map_err(|_| Error::BadMessage)
                }

                dicey_type_DICEY_TYPE_SELECTOR => {
                    let mut selector: dicey_selector = mem::zeroed();

                    ccall!(value_get_selector, &value, &mut selector)?;

                    Ok(ValueView::Selector(Selector::from(selector)))
                }

                dicey_type_DICEY_TYPE_ERROR => {
                    let mut error: dicey_errmsg = mem::zeroed();

                    ccall!(value_get_error, &value, &mut error)?;

                    Ok(ValueView::Error(ErrorMessage::from(error)))
                }

                _ => Err(Error::BadMessage),
            }
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Byte(pub u8);

impl From<u8> for Byte {
    fn from(byte: u8) -> Self {
        Byte(byte)
    }
}

impl From<Byte> for u8 {
    fn from(byte: Byte) -> Self {
        byte.0
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ErrorMessage<'a> {
    pub code: i16,
    pub message: Option<&'a str>,
}

impl From<dicey_errmsg> for ErrorMessage<'_> {
    fn from(c_error: dicey_errmsg) -> Self {
        let message = if c_error.message.is_null() {
            None
        } else {
            Some(unsafe {
                std::ffi::CStr::from_ptr(c_error.message)
                    .to_str()
                    .expect("error messages must be ASCII")
            })
        };

        ErrorMessage {
            code: c_error.code,
            message,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Path<'a>(pub &'a str);

impl Deref for Path<'_> {
    type Target = str;

    fn deref(&self) -> &Self::Target {
        self.0
    }
}

pub(crate) fn bytes_to_cpath(path: impl Into<Vec<u8>>) -> Result<CString, Error> {
    CString::new(path.into()).map_err(|_| Error::MalformedPath)
}

impl<'a> From<&'a str> for Path<'a> {
    fn from(path: &'a str) -> Self {
        Path(path)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Selector<'a> {
    pub trait_name: &'a [u8],
    pub elem: &'a [u8],
}

impl<'a> From<(&'a str, &'a str)> for Selector<'a> {
    fn from((trait_name, elem): (&'a str, &'a str)) -> Self {
        (trait_name.as_bytes(), elem.as_bytes()).into()
    }
}

impl<'a> From<(&'a [u8], &'a [u8])> for Selector<'a> {
    fn from((trait_name, elem): (&'a [u8], &'a [u8])) -> Self {
        Selector { trait_name, elem }
    }
}

impl From<dicey_selector> for Selector<'_> {
    fn from(c_selector: dicey_selector) -> Self {
        let (trait_name, elem) = unsafe {
            (
                CStr::from_ptr(c_selector.trait_).to_bytes(),
                CStr::from_ptr(c_selector.elem).to_bytes(),
            )
        };

        Selector { trait_name, elem }
    }
}

pub trait FromDicey<'a>: Sized {
    fn from_dicey(value: &ValueView<'a>) -> Result<Self, Error>;
}

impl FromDicey<'_> for () {
    fn from_dicey(value: &ValueView<'_>) -> Result<Self, Error> {
        match value {
            ValueView::Unit => Ok(()),
            _ => Err(Error::ValueTypeMismatch),
        }
    }
}

impl FromDicey<'_> for bool {
    fn from_dicey(value: &ValueView<'_>) -> Result<Self, Error> {
        match value {
            ValueView::Bool(b) => Ok(*b),
            _ => Err(Error::ValueTypeMismatch),
        }
    }
}

impl FromDicey<'_> for Byte {
    fn from_dicey(value: &ValueView<'_>) -> Result<Self, Error> {
        match value {
            ValueView::Byte(b) => Ok(*b),
            _ => Err(Error::ValueTypeMismatch),
        }
    }
}

impl FromDicey<'_> for f64 {
    fn from_dicey(value: &ValueView<'_>) -> Result<Self, Error> {
        match value {
            ValueView::Float(f) => Ok(*f),
            _ => Err(Error::ValueTypeMismatch),
        }
    }
}

impl FromDicey<'_> for i16 {
    fn from_dicey(value: &ValueView<'_>) -> Result<Self, Error> {
        match value {
            ValueView::Int16(i) => Ok(*i),
            _ => Err(Error::ValueTypeMismatch),
        }
    }
}

impl FromDicey<'_> for i32 {
    fn from_dicey(value: &ValueView<'_>) -> Result<Self, Error> {
        match value {
            ValueView::Int32(i) => Ok(*i),
            _ => Err(Error::ValueTypeMismatch),
        }
    }
}

impl FromDicey<'_> for i64 {
    fn from_dicey(value: &ValueView<'_>) -> Result<Self, Error> {
        match value {
            ValueView::Int64(i) => Ok(*i),
            _ => Err(Error::ValueTypeMismatch),
        }
    }
}

impl FromDicey<'_> for u16 {
    fn from_dicey(value: &ValueView<'_>) -> Result<Self, Error> {
        match value {
            ValueView::UInt16(u) => Ok(*u),
            _ => Err(Error::ValueTypeMismatch),
        }
    }
}

impl FromDicey<'_> for u32 {
    fn from_dicey(value: &ValueView<'_>) -> Result<Self, Error> {
        match value {
            ValueView::UInt32(u) => Ok(*u),
            _ => Err(Error::ValueTypeMismatch),
        }
    }
}

impl FromDicey<'_> for u64 {
    fn from_dicey(value: &ValueView<'_>) -> Result<Self, Error> {
        match value {
            ValueView::UInt64(u) => Ok(*u),
            _ => Err(Error::ValueTypeMismatch),
        }
    }
}

impl FromDicey<'_> for Uuid {
    fn from_dicey(value: &ValueView<'_>) -> Result<Self, Error> {
        match value {
            ValueView::Uuid(uuid) => Ok(*uuid),
            _ => Err(Error::ValueTypeMismatch),
        }
    }
}

impl<'a, T: FromDicey<'a>> FromDicey<'a> for Vec<T> {
    fn from_dicey(value: &ValueView<'a>) -> Result<Self, Error> {
        match value {
            ValueView::Array { items, .. } => items.iter().map(T::from_dicey).collect(),
            _ => Err(Error::ValueTypeMismatch),
        }
    }
}

macro_rules! impl_from_dicey_tuple {
    ($($name:ident)+) => {
        impl<'a, $($name: FromDicey<'a>),+> FromDicey<'a> for ($($name,)+) {
            fn from_dicey(value: &ValueView<'a>) -> Result<Self, Error> {
                let mut iter = match value {
                    ValueView::Tuple(iter) => iter.iter(),
                    _ => return Err(Error::ValueTypeMismatch),
                };

                Ok(($({
                    // we need this, otherwise the hack below won't work
                    #![allow(non_snake_case)]

                    let $name = $name::from_dicey(iter.next().ok_or(Error::ValueTypeMismatch)?)?;
                    $name
                },)+))
            }
        }
    };
}

// implement FromDicey for tuples of size 1 to 32
impl_from_dicey_tuple!(A B C);
impl_from_dicey_tuple!(A B C D);
impl_from_dicey_tuple!(A B C D E);
impl_from_dicey_tuple!(A B C D E F);
impl_from_dicey_tuple!(A B C D E F G);
impl_from_dicey_tuple!(A B C D E F G H);
impl_from_dicey_tuple!(A B C D E F G H I);
impl_from_dicey_tuple!(A B C D E F G H I J);
impl_from_dicey_tuple!(A B C D E F G H I J K);
impl_from_dicey_tuple!(A B C D E F G H I J K L);
impl_from_dicey_tuple!(A B C D E F G H I J K L M);
impl_from_dicey_tuple!(A B C D E F G H I J K L M N);
impl_from_dicey_tuple!(A B C D E F G H I J K L M N O);
impl_from_dicey_tuple!(A B C D E F G H I J K L M N O P);
impl_from_dicey_tuple!(A B C D E F G H I J K L M N O P Q);
impl_from_dicey_tuple!(A B C D E F G H I J K L M N O P Q R);
impl_from_dicey_tuple!(A B C D E F G H I J K L M N O P Q R S);
impl_from_dicey_tuple!(A B C D E F G H I J K L M N O P Q R S T);
impl_from_dicey_tuple!(A B C D E F G H I J K L M N O P Q R S T U);
impl_from_dicey_tuple!(A B C D E F G H I J K L M N O P Q R S T U V);
impl_from_dicey_tuple!(A B C D E F G H I J K L M N O P Q R S T U V W);
impl_from_dicey_tuple!(A B C D E F G H I J K L M N O P Q R S T U V W X);
impl_from_dicey_tuple!(A B C D E F G H I J K L M N O P Q R S T U V W X Y);
impl_from_dicey_tuple!(A B C D E F G H I J K L M N O P Q R S T U V W X Y Z);
impl_from_dicey_tuple!(A B C D E F G H I J K L M N O P Q R S T U V W X Y Z あ);
impl_from_dicey_tuple!(A B C D E F G H I J K L M N O P Q R S T U V W X Y Z あ い);
impl_from_dicey_tuple!(A B C D E F G H I J K L M N O P Q R S T U V W X Y Z あ い う);
impl_from_dicey_tuple!(A B C D E F G H I J K L M N O P Q R S T U V W X Y Z あ い う え);
impl_from_dicey_tuple!(A B C D E F G H I J K L M N O P Q R S T U V W X Y Z あ い う え お);
impl_from_dicey_tuple!(A B C D E F G H I J K L M N O P Q R S T U V W X Y Z あ い う え お か);

impl<'a, T, U> FromDicey<'a> for (T, U)
where
    T: FromDicey<'a>,
    U: FromDicey<'a>,
{
    fn from_dicey(value: &ValueView<'a>) -> Result<Self, Error> {
        match value {
            ValueView::Pair(a, b) => Ok((T::from_dicey(a)?, U::from_dicey(b)?)),
            _ => Err(Error::ValueTypeMismatch),
        }
    }
}

impl<'a> FromDicey<'a> for &'a [u8] {
    fn from_dicey(value: &ValueView<'a>) -> Result<Self, Error> {
        match value {
            ValueView::Bytes(bytes) => Ok(bytes),
            _ => Err(Error::ValueTypeMismatch),
        }
    }
}

impl<'a> FromDicey<'a> for Vec<u8> {
    fn from_dicey(value: &ValueView<'a>) -> Result<Self, Error> {
        value.extract::<&[u8]>().map(|bytes| bytes.to_owned())
    }
}

impl<'a> FromDicey<'a> for &'a str {
    fn from_dicey(value: &ValueView<'a>) -> Result<Self, Error> {
        match value {
            ValueView::String(s) => Ok(s),
            _ => Err(Error::ValueTypeMismatch),
        }
    }
}

impl<'a> FromDicey<'a> for String {
    fn from_dicey(value: &ValueView<'a>) -> Result<Self, Error> {
        value.extract::<&str>().map(|s| s.to_owned())
    }
}

unsafe fn extract_list<'a>(list: dicey_list) -> Result<Vec<ValueView<'a>>, Error> {
    let mut ret = Vec::new();

    unsafe {
        let mut iter = dicey_list_iter(&list);

        while dicey_iterator_has_next(iter) {
            let mut value = mem::zeroed();

            ccall!(iterator_next, &mut iter, &mut value)?;

            ret.push(ValueView::try_from(value)?);
        }
    }

    Ok(ret)
}

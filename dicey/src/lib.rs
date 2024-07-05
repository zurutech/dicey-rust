// Copyright (c) 2014-2024 Zuru Tech HK Limited, All rights reserved.

#![allow(non_upper_case_globals)]

mod core;
mod ipc;

pub use self::{
    core::{
        errors::*, Bye, FromDicey, Hello, Message, MessageBuilder, Op, Packet, Selector, ToDicey,
        ValueBuilder, ValueView,
    },
    ipc::{blocking, Address, Element, Elements, ObjectInfo, Operation, Property, Signal, Traits},
};

#[cfg(feature = "async")]
pub use self::ipc::{Client, EventSource, RequestBuilder};

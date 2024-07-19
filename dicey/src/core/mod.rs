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

#![allow(non_upper_case_globals)]

pub(crate) mod errors;
pub(crate) mod macros;
pub(crate) mod value;

mod builder;

use std::{
    ffi::c_void,
    fmt, io,
    mem::{self, ManuallyDrop},
    path::Path,
};

use paste::paste;

use dicey_sys::{
    dicey_bye, dicey_bye_reason_DICEY_BYE_REASON_ERROR, dicey_bye_reason_DICEY_BYE_REASON_SHUTDOWN,
    dicey_hello, dicey_message, dicey_op, dicey_op_DICEY_OP_EVENT, dicey_op_DICEY_OP_EXEC,
    dicey_op_DICEY_OP_GET, dicey_op_DICEY_OP_RESPONSE, dicey_op_DICEY_OP_SET, dicey_packet,
    dicey_packet_as_bye, dicey_packet_as_hello, dicey_packet_as_message, dicey_packet_deinit,
    dicey_packet_get_kind, dicey_packet_get_seq, dicey_packet_is_valid,
    dicey_packet_kind_DICEY_PACKET_KIND_BYE, dicey_packet_kind_DICEY_PACKET_KIND_HELLO,
    dicey_packet_kind_DICEY_PACKET_KIND_MESSAGE, dicey_packet_load, dicey_version,
};

pub use self::{
    builder::{MessageBuilder, ToDicey, ValueBuilder},
    errors::Error,
    value::{FromDicey, Selector, ValueView},
};

use self::macros::ccall;

#[derive(Debug)]
pub enum Packet {
    Bye(Bye),
    Hello(Hello),
    Message(Message),
}

impl Packet {
    pub fn load(bytes: &[u8]) -> Result<Self, Error> {
        let pw = RawPacket::load(bytes)?;

        match pw.op() {
            dicey_packet_kind_DICEY_PACKET_KIND_BYE => Ok(Packet::Bye(Bye::try_from(pw)?)),
            dicey_packet_kind_DICEY_PACKET_KIND_HELLO => Ok(Packet::Hello(Hello::try_from(pw)?)),
            dicey_packet_kind_DICEY_PACKET_KIND_MESSAGE => {
                Ok(Packet::Message(Message::try_from(pw)?))
            }
            _ => Err(Error::InvalidData),
        }
    }

    pub fn load_from(mut read: impl io::Read) -> Result<Self, Error> {
        let mut bytes = Vec::new();

        read.read_to_end(&mut bytes).map_err(map_io_error)?;

        Self::load(&bytes)
    }

    pub fn load_path(path: impl AsRef<Path>) -> Result<Self, Error> {
        Self::load(&std::fs::read(path).map_err(map_io_error)?)
    }

    pub fn seq(&self) -> u32 {
        match self {
            Packet::Bye(b) => b.seq(),
            Packet::Hello(h) => h.seq(),
            Packet::Message(m) => m.seq(),
        }
    }
}

pub struct Bye {
    rpacket: RawPacket,

    c_data: dicey_bye,
}

impl Bye {
    pub fn seq(&self) -> u32 {
        self.rpacket.seq()
    }

    const fn reason(&self) -> ByeReason {
        match self.c_data.reason {
            dicey_bye_reason_DICEY_BYE_REASON_SHUTDOWN => ByeReason::Shutdown,
            dicey_bye_reason_DICEY_BYE_REASON_ERROR => ByeReason::Error,
            _ => unreachable!(),
        }
    }
}

impl TryFrom<RawPacket> for Bye {
    type Error = Error;

    fn try_from(rpacket: RawPacket) -> Result<Self, Self::Error> {
        unsafe {
            if rpacket.op() != dicey_packet_kind_DICEY_PACKET_KIND_BYE {
                return Err(Error::InvalidData);
            }

            let mut c_data = mem::zeroed();

            ccall!(packet_as_bye, rpacket.content, &mut c_data)?;

            // validate the reason
            ByeReason::try_from(c_data.reason)?;

            Ok(Bye { rpacket, c_data })
        }
    }
}

impl fmt::Debug for Bye {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Bye")
            .field("seq", &self.seq())
            .field("reason", &self.reason())
            .finish()
    }
}

#[repr(u8)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ByeReason {
    Shutdown = dicey_bye_reason_DICEY_BYE_REASON_SHUTDOWN as u8,
    Error = dicey_bye_reason_DICEY_BYE_REASON_ERROR as u8,
}

impl TryFrom<u32> for ByeReason {
    type Error = Error;

    fn try_from(value: u32) -> Result<Self, Error> {
        match value {
            dicey_bye_reason_DICEY_BYE_REASON_SHUTDOWN => Ok(ByeReason::Shutdown),
            dicey_bye_reason_DICEY_BYE_REASON_ERROR => Ok(ByeReason::Error),
            _ => Err(Error::InvalidData),
        }
    }
}

pub struct Event(RawMessage);

impl Event {
    pub fn seq(&self) -> u32 {
        self.0.seq()
    }

    pub fn path(&self) -> &str {
        self.0.path()
    }

    pub fn selector(&self) -> Selector {
        self.0.selector()
    }

    pub fn value(&self) -> ValueView {
        self.0.value()
    }

    pub(crate) fn into_raw(self) -> dicey_packet {
        self.0.into_raw()
    }
}

impl fmt::Debug for Event {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.debug(f)
    }
}

pub struct Exec(RawMessage);

impl Exec {
    pub fn seq(&self) -> u32 {
        self.0.seq()
    }

    pub fn path(&self) -> &str {
        self.0.path()
    }

    pub fn selector(&self) -> Selector {
        self.0.selector()
    }

    pub fn value(&self) -> ValueView {
        self.0.value()
    }

    pub(crate) fn into_raw(self) -> dicey_packet {
        self.0.into_raw()
    }
}

impl fmt::Debug for Exec {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.debug(f)
    }
}

pub struct Get(RawMessage);

impl Get {
    pub fn seq(&self) -> u32 {
        self.0.seq()
    }

    pub fn path(&self) -> &str {
        self.0.path()
    }

    pub fn selector(&self) -> Selector {
        self.0.selector()
    }

    pub(crate) fn into_raw(self) -> dicey_packet {
        self.0.into_raw()
    }
}

impl fmt::Debug for Get {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.debug(f)
    }
}

pub struct Hello {
    rpacket: RawPacket,

    c_data: dicey_hello,
}

impl Hello {
    pub fn seq(&self) -> u32 {
        self.rpacket.seq()
    }

    pub fn version(&self) -> Version {
        self.c_data.version.into()
    }
}

impl TryFrom<RawPacket> for Hello {
    type Error = Error;

    fn try_from(rpacket: RawPacket) -> Result<Self, Self::Error> {
        unsafe {
            if rpacket.op() != dicey_packet_kind_DICEY_PACKET_KIND_HELLO {
                return Err(Error::InvalidData);
            }

            let mut c_data = mem::zeroed();

            ccall!(packet_as_hello, rpacket.content, &mut c_data)?;

            Ok(Hello { rpacket, c_data })
        }
    }
}

impl fmt::Debug for Hello {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Hello")
            .field("seq", &self.seq())
            .field("version", &self.version())
            .finish()
    }
}

#[derive(Debug)]
pub enum Message {
    Event(Event),
    Exec(Exec),
    Get(Get),
    Response(Response),
    Set(Set),
}

impl Message {
    pub const fn op(&self) -> Op {
        match self {
            Message::Event(_) => Op::Event,
            Message::Exec(_) => Op::Exec,
            Message::Get(_) => Op::Get,
            Message::Response(_) => Op::Response,
            Message::Set(_) => Op::Set,
        }
    }

    pub fn path(&self) -> &str {
        match self {
            Message::Event(e) => e.path(),
            Message::Exec(e) => e.path(),
            Message::Get(e) => e.path(),
            Message::Response(e) => e.path(),
            Message::Set(e) => e.path(),
        }
    }

    pub fn selector(&self) -> Selector {
        match self {
            Message::Event(e) => e.selector(),
            Message::Exec(e) => e.selector(),
            Message::Get(e) => e.selector(),
            Message::Response(e) => e.selector(),
            Message::Set(e) => e.selector(),
        }
    }

    pub fn seq(&self) -> u32 {
        match self {
            Message::Event(e) => e.seq(),
            Message::Exec(e) => e.seq(),
            Message::Get(e) => e.seq(),
            Message::Response(e) => e.seq(),
            Message::Set(e) => e.seq(),
        }
    }

    pub fn value(&self) -> Option<ValueView> {
        match self {
            Message::Event(e) => Some(e.value()),
            Message::Exec(e) => Some(e.value()),
            Message::Get(_) => None,
            Message::Response(e) => Some(e.value()),
            Message::Set(e) => Some(e.value()),
        }
    }

    pub(crate) fn from_raw(cpacket: dicey_packet) -> Result<Self, Error> {
        RawPacket::from(cpacket).try_into()
    }

    pub(crate) fn into_raw(self) -> dicey_packet {
        match self {
            Message::Event(e) => e.into_raw(),
            Message::Exec(e) => e.into_raw(),
            Message::Get(e) => e.into_raw(),
            Message::Response(e) => e.into_raw(),
            Message::Set(e) => e.into_raw(),
        }
    }
}

impl TryFrom<RawMessage> for Message {
    type Error = Error;

    fn try_from(rmessage: RawMessage) -> Result<Self, Self::Error> {
        match rmessage.op() {
            dicey_op_DICEY_OP_EVENT => Ok(Message::Event(Event(rmessage))),
            dicey_op_DICEY_OP_EXEC => Ok(Message::Exec(Exec(rmessage))),
            dicey_op_DICEY_OP_GET => Ok(Message::Get(Get(rmessage))),
            dicey_op_DICEY_OP_RESPONSE => Ok(Message::Response(Response(rmessage))),
            dicey_op_DICEY_OP_SET => Ok(Message::Set(Set(rmessage))),

            _ => panic!("bug in C libdicey: validation failed"),
        }
    }
}

impl TryFrom<RawPacket> for Message {
    type Error = Error;

    fn try_from(rpacket: RawPacket) -> Result<Self, Self::Error> {
        RawMessage::try_from(rpacket).and_then(Message::try_from)
    }
}

#[derive(Clone, Copy, Debug)]
pub enum Op {
    Event,
    Exec,
    Get,
    Response,
    Set,
}

impl Op {
    pub(crate) const fn as_c(self) -> dicey_op {
        match self {
            Op::Event => dicey_op_DICEY_OP_EVENT,
            Op::Exec => dicey_op_DICEY_OP_EXEC,
            Op::Get => dicey_op_DICEY_OP_GET,
            Op::Response => dicey_op_DICEY_OP_RESPONSE,
            Op::Set => dicey_op_DICEY_OP_SET,
        }
    }
}

pub struct Response(RawMessage);

impl Response {
    pub fn seq(&self) -> u32 {
        self.0.seq()
    }

    pub fn path(&self) -> &str {
        self.0.path()
    }

    pub fn selector(&self) -> Selector {
        self.0.selector()
    }

    pub fn value(&self) -> ValueView {
        self.0.value()
    }

    pub(crate) fn into_raw(self) -> dicey_packet {
        self.0.into_raw()
    }
}

impl fmt::Debug for Response {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.debug(f)
    }
}

pub struct Set(RawMessage);

impl Set {
    pub fn seq(&self) -> u32 {
        self.0.seq()
    }

    pub fn path(&self) -> &str {
        self.0.path()
    }

    pub fn selector(&self) -> Selector {
        self.0.selector()
    }

    pub fn value(&self) -> ValueView {
        self.0.value()
    }

    pub(crate) fn into_raw(self) -> dicey_packet {
        self.0.into_raw()
    }
}

impl fmt::Debug for Set {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.debug(f)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Version {
    pub major: u16,
    pub revision: u16,
}

impl From<dicey_version> for Version {
    fn from(c_version: dicey_version) -> Self {
        Version {
            major: c_version.major,
            revision: c_version.revision,
        }
    }
}

struct RawMessage {
    rpacket: RawPacket,

    c_data: dicey_message,
}

impl RawMessage {
    const fn op(&self) -> u32 {
        self.c_data.type_
    }

    const fn op_name(&self) -> &'static str {
        match self.op() {
            dicey_op_DICEY_OP_EVENT => "Event",
            dicey_op_DICEY_OP_EXEC => "Exec",
            dicey_op_DICEY_OP_GET => "Get",
            dicey_op_DICEY_OP_RESPONSE => "Response",
            dicey_op_DICEY_OP_SET => "Set",
            _ => unreachable!(),
        }
    }

    fn debug(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct(self.op_name())
            .field("seq", &self.seq())
            .field("path", &self.path())
            .field("selector", &self.selector())
            .field("value", &self.value())
            .finish()
    }

    fn into_raw(self) -> dicey_packet {
        // return the C packet and present self from being dropped
        self.rpacket.into_raw()
    }

    fn path(&self) -> &str {
        unsafe { std::ffi::CStr::from_ptr(self.c_data.path) }
            .to_str()
            .expect("the path must be ASCII")
    }

    fn selector(&self) -> Selector {
        Selector::from(self.c_data.selector)
    }

    fn seq(&self) -> u32 {
        self.rpacket.seq()
    }

    fn value(&self) -> ValueView {
        ValueView::try_from(self.c_data.value)
            .expect("the value must be valid, this is probably a C bug")
    }
}

impl TryFrom<RawPacket> for RawMessage {
    type Error = Error;

    fn try_from(rpacket: RawPacket) -> Result<Self, Self::Error> {
        unsafe {
            if rpacket.op() != dicey_packet_kind_DICEY_PACKET_KIND_MESSAGE {
                return Err(Error::InvalidData);
            }

            let mut c_data = mem::zeroed();

            ccall!(packet_as_message, rpacket.content, &mut c_data)?;

            Ok(RawMessage { rpacket, c_data })
        }
    }
}

// RawMessage is send and sync because it's literally a pointer to a byte array plus a C struct containing some
// pointers into the same byte array
unsafe impl Send for RawMessage {}
unsafe impl Sync for RawMessage {}

pub(crate) struct RawPacket {
    content: dicey_packet,
}

impl RawPacket {
    fn load(bytes: &[u8]) -> Result<Self, Error> {
        let mut bytes_void = bytes.as_ptr() as *const c_void;
        let mut bytes_len = bytes.len();

        unsafe {
            let mut rpacket = mem::zeroed();

            ccall!(packet_load, &mut rpacket, &mut bytes_void, &mut bytes_len)?;

            Ok(RawPacket { content: rpacket })
        }
    }

    fn into_raw(self) -> dicey_packet {
        // return the C packet and present self from being dropped
        ManuallyDrop::new(self).content
    }

    fn seq(&self) -> u32 {
        let mut seq = 0u32;

        unsafe { ccall!(packet_get_seq, self.content, &mut seq) }
            .expect("the packet must either be valid or get rejected");

        seq
    }

    fn op(&self) -> u32 {
        unsafe { dicey_packet_get_kind(self.content) }
    }
}

impl Drop for RawPacket {
    fn drop(&mut self) {
        unsafe {
            dicey_packet_deinit(&mut self.content);
        }
    }
}

impl From<dicey_packet> for RawPacket {
    fn from(cpacket: dicey_packet) -> Self {
        assert!(unsafe { dicey_packet_is_valid(cpacket) });

        RawPacket { content: cpacket }
    }
}

// RawPacket is send and sync because it's literally a pointer to a byte array
unsafe impl Send for RawPacket {}
unsafe impl Sync for RawPacket {}

fn map_io_error(err: io::Error) -> Error {
    match err.kind() {
        io::ErrorKind::NotFound => Error::FileNotFound,
        io::ErrorKind::InvalidInput | io::ErrorKind::InvalidData => Error::InvalidData,
        io::ErrorKind::ConnectionRefused => Error::ConnectionRefused,
        io::ErrorKind::ConnectionReset => Error::ConnectionReset,
        io::ErrorKind::TimedOut => Error::TimedOut,
        io::ErrorKind::AlreadyExists => Error::Already,
        io::ErrorKind::BrokenPipe => Error::BrokenPipe,

        _ => Error::UnknownUVError, // fine for now
    }
}

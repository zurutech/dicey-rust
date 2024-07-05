pub mod blocking;

mod address;
mod object_info;

pub use address::Address;
pub use object_info::{Element, Elements, ObjectInfo, Operation, Property, Signal, Traits};

#[cfg(feature = "async")]
mod client;

#[cfg(feature = "async")]
pub use client::{Client, EventSource, RequestBuilder};

pub const DEFAULT_TIMEOUT_MS: u32 = 1000u32;

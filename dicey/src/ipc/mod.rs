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

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

use std::error;

use clap::Parser;

use dicey::{
    Message,
    blocking::{Client, ClientArgs},
};
use uuid::Uuid;

#[derive(Parser)]
struct Opts {
    socket: String,
}

const ECHO_PATH: &str = "/dicey/test/echo";
const ECHO_TRAIT: &str = "dicey.test.Echo";
const ECHO_ECHO_ELEMENT: &str = "Echo";

fn main() -> Result<(), Box<dyn error::Error>> {
    let opts = Opts::parse();

    let cln = Client::connect(ClientArgs {
        pipe: &opts.socket,
        on_event: Some(|m: Message| {
            println!("received event: {m:?}");
        }),
    })?;

    let uuid = Uuid::new_v4();

    println!("uuid (send) = {uuid}");

    let response: Uuid = cln
        .exec(ECHO_PATH, (ECHO_TRAIT, ECHO_ECHO_ELEMENT), uuid)?
        .value()
        .unwrap()
        .extract()?;

    println!("uuid (recv) = {response}");

    Ok(())
}

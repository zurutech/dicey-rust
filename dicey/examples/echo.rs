use std::error;

use clap::Parser;

use dicey::{
    blocking::{Client, ClientArgs},
    Message,
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

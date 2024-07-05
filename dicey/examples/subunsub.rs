use std::{error, time::Duration};

use clap::Parser;

use tokio::time::sleep;

use dicey::Client;

#[derive(Parser)]
struct Opts {
    socket: String,
    seconds: u64,
}

const TEST_TIMER_PATH: &str = "/dicey/test/timer";
const TEST_TIMER_TRAIT: &str = "dicey.test.Timer";
const TEST_TIMER_START_ELEMENT: &str = "Start";
const TEST_TIMER_TIMERFIRED_ELEMENT: &str = "TimerFired";

#[tokio::main]
async fn main() -> Result<(), Box<dyn error::Error>> {
    let opts = Opts::parse();

    let cln = Client::connect(&opts.socket).await?;
    let mut events = cln.events();

    let _ = tokio::spawn(async move {
        while let Ok(msg) = events.next().await {
            println!("received event: {msg:?}");
        }
    });

    cln.subscribe_to(
        TEST_TIMER_PATH,
        (TEST_TIMER_TRAIT, TEST_TIMER_TIMERFIRED_ELEMENT),
    )
    .await?;

    let _: () = cln
        .exec(
            TEST_TIMER_PATH,
            (TEST_TIMER_TRAIT, TEST_TIMER_START_ELEMENT),
            i32::try_from(opts.seconds)?,
        )
        .await?
        .value()
        .unwrap()
        .extract()?;

    sleep(Duration::from_secs(opts.seconds + 1)).await;

    Ok(())
}

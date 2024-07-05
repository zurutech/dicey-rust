use std::{error, time::Instant};

use clap::Parser;

use dicey::{
    blocking::{Client, ClientArgs},
    Message,
};

#[derive(Parser)]
struct Opts {
    socket: String,
    value: Option<String>,

    #[clap(short, long)]
    time: bool,
}

const SVAL_PATH: &str = "/sval";
const SVAL_TRAIT: &str = "sval.Sval";
const SVAL_PROP: &str = "Value";

fn estimate(reqtime_us: f64) -> (f64, Option<char>) {
    let req_s = 1000000. / reqtime_us;

    if req_s > 1000000000. {
        return (req_s / 1000000000., Some('G'));
    }

    if req_s > 1000000. {
        return (req_s / 1000000., Some('M'));
    }

    if req_s > 1000. {
        return (req_s / 1000., Some('k'));
    }

    return (req_s, None);
}

fn main() -> Result<(), Box<dyn error::Error>> {
    let opts = Opts::parse();

    let cln = Client::connect(ClientArgs {
        pipe: &opts.socket,
        on_event: Some(|m: Message| {
            println!("received event: {m:?}");
        }),
    })?;

    let start = Instant::now();
    let elapsed = if let Some(value) = opts.value {
        cln.set(SVAL_PATH, (SVAL_TRAIT, SVAL_PROP), value)?;

        start.elapsed()
    } else {
        let sval: String = cln
            .get(SVAL_PATH, (SVAL_TRAIT, SVAL_PROP))?
            .value()
            .unwrap()
            .extract()?;

        let end = start.elapsed();

        println!(r#"{SVAL_PATH}#{SVAL_TRAIT}.{SVAL_PROP} = "{sval}""#);

        end
    }
    .as_micros();

    if opts.time {
        let (reqtime, unit) = estimate(elapsed as f64);

        if let Some(unit) = unit {
            println!("reqtime: {elapsed}us ({reqtime:.2} {unit}req/s)");
        } else {
            println!("reqtime: {elapsed}us ({reqtime:.2} req/s)");
        }
    }

    Ok(())
}

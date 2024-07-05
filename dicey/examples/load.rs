use std::{error, path::PathBuf};

use clap::Parser;

use dicey::Packet;

#[derive(Parser)]
struct Opts {
    file: PathBuf,
}

fn main() -> Result<(), Box<dyn error::Error>> {
    let opts = Opts::parse();

    println!("{:#?}", Packet::load_path(opts.file)?);

    Ok(())
}

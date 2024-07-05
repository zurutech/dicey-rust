use std::error;

use clap::Parser;

use dicey::Client;

#[derive(Parser)]
struct Opts {
    socket: String,
    path: String,
}

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

    println!("INSPECT {}", opts.path);

    println!("Data = {:?}", cln.inspect(opts.path.clone()).await?);

    let xml: String = cln.inspect_as_xml(opts.path).await?;

    println!("XML = {xml}");

    Ok(())
}

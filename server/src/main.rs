#[macro_use] extern crate rocket;
use crate::server::create_server;
use clap::Parser;

pub mod handlers;
pub mod llm;
pub mod server;
pub mod search;
pub mod agent_search;

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
struct Args {
    #[arg(short, long, default_value_t = 8095)]
    port: u16,
}

#[rocket::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();
    let config = rocket::Config::figment()
        .merge(("port", args.port));
    create_server()
        .configure(config)
        .launch()
        .await?;
    Ok(())
}

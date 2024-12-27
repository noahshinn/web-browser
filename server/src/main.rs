use crate::server::{create_server, run_server};
use std::env;

pub mod agent_search;
pub mod handlers;
pub mod llm;
pub mod prompts;
pub mod query;
pub mod result_format;
pub mod search;
pub mod server;
pub mod utils;
pub mod webpage_parse;

#[rocket::main]
async fn main() {
    if let Err(e) = run().await {
        eprintln!("Server error: {}", e);
        std::process::exit(1);
    }
}

async fn run() -> Result<(), server::ServerError> {
    let port: u16 = env::var("WEB_SEARCH_SERVER_PORT")
        .ok()
        .and_then(|p| p.parse().ok())
        .unwrap_or(8095);

    let config = rocket::Config::figment().merge(("port", port));

    let rocket = create_server()?.configure(config);

    run_server(rocket).await
}

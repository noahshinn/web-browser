use crate::server::create_server;
use std::env;

pub mod handlers;
pub mod llm;
pub mod server;
pub mod search;
pub mod agent_search;
pub mod utils;

#[rocket::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let port: u16 = env::var("WEB_SEARCH_SERVER_PORT")
        .ok()
        .and_then(|p| p.parse().ok())
        .unwrap_or(8095);

    let config = rocket::Config::figment()
        .merge(("port", port));

    create_server()
        .configure(config)
        .launch()
        .await?;
    Ok(())
}

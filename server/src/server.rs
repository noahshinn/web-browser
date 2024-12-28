use crate::handlers::agent_search::handle_agent_search;
use crate::handlers::scrape_site::handle_scrape_site;
use crate::handlers::search::handle_search;
use rocket::routes;

#[derive(Debug)]
pub enum ServerError {
    Launch(rocket::Error),
    Configuration(String),
    Environment(std::env::VarError),
}

impl std::error::Error for ServerError {}

impl std::fmt::Display for ServerError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ServerError::Launch(e) => write!(f, "Server launch error: {}", e),
            ServerError::Configuration(e) => write!(f, "Configuration error: {}", e),
            ServerError::Environment(e) => write!(f, "Environment variable error: {}", e),
        }
    }
}

pub struct ServerState {
    pub searx_host: String,
    pub searx_port: String,
}

pub fn create_server() -> Result<rocket::Rocket<rocket::Build>, ServerError> {
    let searx_host = std::env::var("SEARX_HOST").unwrap_or_else(|_| "localhost".to_string());
    let searx_port = std::env::var("SEARX_PORT").unwrap_or_else(|_| "8096".to_string());

    Ok(rocket::build()
        .manage(ServerState {
            searx_host: searx_host,
            searx_port: searx_port,
        })
        .mount(
            "/",
            routes![handle_search, handle_agent_search, handle_scrape_site],
        ))
}

pub async fn run_server(rocket: rocket::Rocket<rocket::Build>) -> Result<(), ServerError> {
    match rocket.launch().await {
        Ok(_) => Ok(()),
        Err(e) => {
            eprintln!("Failed to launch rocket server: {}", e);
            Err(ServerError::Launch(e))
        }
    }
}

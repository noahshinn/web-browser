use crate::handlers::search::search;
use crate::handlers::agent_search::agent_search;
use rocket::routes;

pub struct ServerState {
    pub searx_host: String,
    pub searx_port: String,
}

pub fn create_server() -> rocket::Rocket<rocket::Build> {
    let searx_host = std::env::var("SEARX_HOST").unwrap_or_else(|_| "localhost".to_string());
    let searx_port = std::env::var("SEARX_PORT").unwrap_or_else(|_| "8096".to_string());

    rocket::build()
        .manage(ServerState { searx_host, searx_port })
        .mount("/", routes![
            search,
            agent_search,
        ])
}

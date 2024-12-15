use crate::handlers::search::search;
use crate::handlers::agent_search::agent_search;

pub struct ServerState {
    pub searx_host: String,
    pub searx_port: String,
}

pub fn create_server() -> rocket::Rocket<rocket::Build> {
    let searx_host = std::env::var("SEARX_HOST").expect("SEARX_HOST must be set");
    let searx_port = std::env::var("SEARX_PORT").expect("SEARX_PORT must be set");

    rocket::build()
        .manage(ServerState { searx_host, searx_port })
        .mount("/", routes![
            search,
            agent_search,
        ])
}

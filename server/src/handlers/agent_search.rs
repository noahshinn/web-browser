use crate::agent_search::{agent_search, AgentSearchResult, SearchInput};
use crate::server::ServerState;
use rocket::http::Status;
use rocket::post;
use rocket::serde::json::Json;
use rocket::State;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct AgentSearchErrorResponse {
    pub message: String,
    pub error_type: String,
}

#[post("/v1/agent_search", data = "<search_input>")]
pub async fn handle_agent_search(
    state: &State<ServerState>,
    search_input: Json<SearchInput>,
) -> Result<Json<AgentSearchResult>, (Status, Json<AgentSearchErrorResponse>)> {
    match agent_search(&search_input, &state.searx_host, &state.searx_port).await {
        Ok(result) => Ok(Json(result)),
        Err(e) => Err((
            Status::BadRequest,
            Json(AgentSearchErrorResponse {
                message: e.to_string(),
                error_type: "search_error".to_string(),
            }),
        )),
    }
}

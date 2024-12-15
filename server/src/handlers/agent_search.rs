use rocket::get;
use rocket::serde::json::Json;
use rocket::State;
use crate::server::ServerState;
use crate::agent_search::{AgentSearchResult, perform_agent_search};
use crate::search::SearchQuery;
use serde::{Serialize, Deserialize};


#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum AgentSearchResponse {
    #[serde(rename = "success")]
    Success {
        query: String,
        results: AgentSearchResult,
    },
    #[serde(rename = "error")]
    Error {
        message: String,
        error_type: String,
    },
}

#[get("/v1/agent_search?<query..>")]
pub async fn agent_search(state: &State<ServerState>, query: SearchQuery) -> Json<AgentSearchResponse> {
    let result = match perform_agent_search(&query.query, &state.searx_host, &state.searx_port).await {
        Ok(result) => AgentSearchResponse::Success {
            query: query.query,
            results: result,
        },
        Err(e) => AgentSearchResponse::Error {
            message: e.to_string(),
            error_type: "search_error".to_string(),
        },
    };
    Json(result)
}

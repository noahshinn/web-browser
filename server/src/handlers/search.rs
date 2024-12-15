use rocket::get;
use rocket::serde::json::Json;
use rocket::State;
use crate::server::ServerState;
use crate::search::{SearchQuery, SearchResult, search, SearchError};
use serde::{Serialize, Deserialize};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum SearchResponse {
    Success {
        query: String,
        results: Vec<SearchResult>,
    },
    Error {
        message: String,
        error_type: String,
    },
}

#[get("/v1/search?<query..>")]
pub async fn handle_search(state: &State<ServerState>, query: SearchQuery) -> Json<SearchResponse> {
    Json(match search(&query.query, &state.searx_host, &state.searx_port).await {
        Ok(results) => SearchResponse::Success {
            query: query.query,
            results: results,
        },
        Err(e) => SearchResponse::Error {
            message: e.to_string(),
            error_type: match e {
                SearchError::RequestError(_) => "request_error".to_string(),
                SearchError::InvalidSearxUrl { .. } => "invalid_url".to_string(),
                SearchError::SearxError(_) => "searx_error".to_string(),
            },
        }
    })
}

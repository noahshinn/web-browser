use crate::search::{search, SearchError, SearchInput, SearchResult};
use crate::server::ServerState;
use rocket::http::Status;
use rocket::post;
use rocket::serde::json::Json;
use rocket::State;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct SearchErrorResponse {
    pub message: String,
    pub error_type: String,
}

#[post("/search", data = "<search_input>")]
pub async fn handle_search(
    state: &State<ServerState>,
    search_input: Json<SearchInput>,
) -> Result<Json<Vec<SearchResult>>, (Status, Json<SearchErrorResponse>)> {
    match search(&search_input, &state.searx_host, &state.searx_port).await {
        Ok(results) => Ok(Json(results)),
        Err(e) => Err((
            Status::BadRequest,
            Json(SearchErrorResponse {
                message: e.to_string(),
                error_type: match e {
                    SearchError::RequestError(_) => "request_error".to_string(),
                    SearchError::InvalidSearxUrl { .. } => "invalid_url".to_string(),
                    SearchError::SearxError(_) => "searx_error".to_string(),
                },
            }),
        )),
    }
}

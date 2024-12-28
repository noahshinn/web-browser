use crate::scrape_site::{scrape_site, ScrapeSiteError, ScrapeSiteInput, ScrapeSiteResult};
use crate::server::ServerState;
use rocket::http::Status;
use rocket::post;
use rocket::serde::json::Json;
use rocket::State;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ScrapeSiteResponse {
    pub results: Vec<ScrapeSiteResult>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ScrapeSiteErrorResponse {
    pub message: String,
    pub error_type: String,
}

#[post("/scrape_site", data = "<scrape_site_input>")]
pub async fn handle_scrape_site(
    state: &State<ServerState>,
    scrape_site_input: Json<ScrapeSiteInput>,
) -> Result<Json<ScrapeSiteResponse>, (Status, Json<ScrapeSiteErrorResponse>)> {
    match scrape_site(&scrape_site_input, &state.searx_host, &state.searx_port).await {
        Ok(results) => Ok(Json(ScrapeSiteResponse { results })),
        Err(e) => Err((
            Status::BadRequest,
            Json(ScrapeSiteErrorResponse {
                message: e.to_string(),
                error_type: match e {
                    ScrapeSiteError::SearchError(_) => "search_error".to_string(),
                    ScrapeSiteError::FormatError(_) => "format_error".to_string(),
                    ScrapeSiteError::WebpageParseError(_) => "webpage_parse_error".to_string(),
                },
            }),
        )),
    }
}

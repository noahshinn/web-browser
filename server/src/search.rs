use rocket::form::FromForm;
use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(FromForm, Deserialize, Debug, Clone)]
pub struct SearchQuery {
    pub query: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct SearchResult {
    pub title: String,
    pub url: String,
    pub content: String,
}

impl std::fmt::Display for SearchResult {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "# Search result: {} ({})\n\n{}",
            self.title, self.url, self.content
        )
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct SearxSearchResult {
    pub category: Option<String>,
    pub content: String,
    pub engine: Option<String>,
    pub engines: Option<Vec<String>>,
    pub parsed_url: Option<Vec<String>>,
    pub positions: Option<Vec<i32>>,
    pub pretty_url: Option<String>,
    pub score: Option<f64>,
    pub title: String,
    pub url: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct SearxResponse {
    pub answers: Vec<String>,
    pub corrections: Vec<String>,
    pub infoboxes: Vec<String>,
    pub number_of_results: f64,
    pub query: String,
    pub results: Vec<SearxSearchResult>,
}

#[derive(Error, Debug)]
pub enum SearchError {
    #[error("HTTP request failed: {0}")]
    RequestError(#[from] reqwest::Error),
    #[error("Invalid searx URL: {host}:{port}")]
    InvalidSearxUrl { host: String, port: u16 },
    #[error("Searx returned error: {0}")]
    SearxError(String),
}

pub async fn search(
    query: &str,
    searx_host: &str,
    searx_port: &str,
) -> Result<Vec<SearchResult>, SearchError> {
    if query.trim().is_empty() {
        return Ok(vec![]);
    }
    let searx_url = format!("http://{}:{}/search", searx_host, searx_port);
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .map_err(SearchError::RequestError)?;
    let response = client
        .get(&searx_url)
        .query(&[
            ("q", query),
            ("format", "json"),
            ("language", "en"),
            ("engines", "google"),
        ])
        .send()
        .await
        .map_err(SearchError::RequestError)?;
    if !response.status().is_success() {
        return Err(SearchError::SearxError(format!(
            "Searx returned status code: {}",
            response.status()
        )));
    }
    let searx_response = response
        .json::<SearxResponse>()
        .await
        .map_err(SearchError::RequestError)?;
    Ok(searx_response
        .results
        .into_iter()
        .map(|result| SearchResult {
            title: result.title,
            url: result.url,
            content: result.content,
        })
        .collect())
}

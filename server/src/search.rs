use futures::future::join_all;
use rocket::form::FromForm;
use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(FromForm, Deserialize, Debug, Clone)]
pub struct SearchInput {
    pub query: String,
    #[serde(default)]
    pub max_results_to_visit: Option<usize>,
    #[serde(default)]
    pub whitelisted_base_urls: Option<Vec<String>>,
    #[serde(default)]
    pub blacklisted_base_urls: Option<Vec<String>>,
}

impl Default for SearchInput {
    fn default() -> Self {
        Self {
            query: String::new(),
            max_results_to_visit: Some(10),
            whitelisted_base_urls: None,
            blacklisted_base_urls: None,
        }
    }
}

impl SearchInput {
    pub fn build_google_search_query(&self) -> String {
        build_google_search_query(
            &self.query,
            self.whitelisted_base_urls.as_ref(),
            self.blacklisted_base_urls.as_ref(),
        )
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, FromForm)]
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

async fn single_page_search(
    query: &str,
    searx_host: &str,
    searx_port: &str,
    pageno: usize,
) -> Result<Vec<SearchResult>, SearchError> {
    let searx_url = format!("http://{}:{}/search", searx_host, searx_port);
    let client = match reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(60))
        .build()
    {
        Ok(client) => client,
        Err(e) => return Err(SearchError::RequestError(e)),
    };
    let response = client
        .get(&searx_url)
        .query(&[
            ("q", query),
            ("format", "json"),
            ("language", "en"),
            ("engines", "google"),
            ("pageno", pageno.to_string().as_str()),
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
    let searx_response = match response
        .json::<SearxResponse>()
        .await
        .map_err(SearchError::RequestError)
    {
        Ok(searx_response) => searx_response,
        Err(e) => return Err(e),
    };
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

pub const MAX_RESULTS_TO_VISIT: usize = 10;
pub const SEARX_RESULTS_PER_PAGE: usize = 8;

pub async fn search(
    search_input: &SearchInput,
    searx_host: &str,
    searx_port: &str,
) -> Result<Vec<SearchResult>, SearchError> {
    let max_results = search_input
        .max_results_to_visit
        .unwrap_or(MAX_RESULTS_TO_VISIT);
    let num_pages = (max_results + SEARX_RESULTS_PER_PAGE - 1) / SEARX_RESULTS_PER_PAGE;
    let query = search_input.build_google_search_query();
    let futures: Vec<_> = (1..=num_pages)
        .map(|pageno| single_page_search(&query, searx_host, searx_port, pageno))
        .collect();
    let results = join_all(futures).await;
    let mut all_results = Vec::new();
    for page_result in results {
        match page_result {
            Ok(page_results) => {
                for result in page_results {
                    if all_results.len() >= max_results {
                        break;
                    }
                    all_results.push(result);
                }
            }
            Err(e) => return Err(e),
        }
    }
    Ok(all_results)
}

pub fn build_google_search_query(
    query: &str,
    whitelisted_base_urls: Option<&Vec<String>>,
    blacklisted_base_urls: Option<&Vec<String>>,
) -> String {
    let mut parts = vec![query.to_string()];
    if let Some(whitelist) = whitelisted_base_urls {
        let site_query = whitelist
            .iter()
            .map(|url| format!("site:{}", url))
            .collect::<Vec<_>>()
            .join(" OR ");
        parts.push(site_query);
    }
    if let Some(blacklist) = blacklisted_base_urls {
        for url in blacklist {
            parts.push(format!("-site:{}", url));
        }
    }
    parts.join(" ")
}

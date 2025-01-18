use crate::llm::{default_completion, CompletionBuilder, LLMError};
use crate::prompts::{Prompt, SCRAPE_SITE_RESULT_FORMAT_MD_SYSTEM_PROMPT};
use crate::search::{search, SearchError, SearchInput, SearchResult};
use crate::utils::{parse_json_response, ParseJsonError};
use crate::webpage_parse::{visit_and_parse_webpage, ParsedWebpage, WebpageParseError};
use futures::stream::{self, StreamExt};
use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ScrapeSiteInput {
    pub base_url: String,
    pub max_num_pages_to_visit: Option<usize>,
    pub result_format: Option<ScrapeSiteResultFormat>,
    pub max_concurrency: Option<usize>,
}

const DEFAULT_MAX_CONCURRENCY: usize = 10;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ScrapeSiteResult {
    pub search_result: SearchResult,
    pub formatted_content: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum ScrapeSiteResultFormat {
    #[serde(rename = "html")]
    Html,
    #[serde(rename = "md")]
    Md,
}

impl Default for ScrapeSiteResultFormat {
    fn default() -> Self {
        ScrapeSiteResultFormat::Html
    }
}

#[derive(Error, Debug)]
pub enum ScrapeSiteError {
    #[error("Search returned error: {0}")]
    SearchError(#[from] SearchError),
    #[error("Failed to format result with llm: {0}")]
    FormatError(#[from] ScrapeSiteFormatError),
    #[error("Failed to parse webpage: {0}")]
    WebpageParseError(#[from] WebpageParseError),
}

const MAX_NUM_PAGES_TO_VISIT: usize = 2000;

struct ParsedSearchResult {
    pub search_result: SearchResult,
    pub parsed_webpage: ParsedWebpage,
}

pub async fn scrape_site(
    scrape_input: &ScrapeSiteInput,
    searx_host: &str,
    searx_port: &str,
) -> Result<Vec<ScrapeSiteResult>, ScrapeSiteError> {
    let num_pages = scrape_input
        .max_num_pages_to_visit
        .unwrap_or(MAX_NUM_PAGES_TO_VISIT);
    let search_input = SearchInput {
        query: "".to_string(),
        max_results_to_visit: Some(num_pages),
        whitelisted_base_urls: Some(vec![scrape_input.base_url.clone()]),
        blacklisted_base_urls: None,
    };
    let json_results = match search(&search_input, searx_host, searx_port).await {
        Ok(results) => results,
        Err(e) => return Err(ScrapeSiteError::SearchError(e)),
    };
    let futures = json_results
        .into_iter()
        .map(|result| async {
            let parsed_webpage = visit_and_parse_webpage(&result.url).await;
            ParsedSearchResult {
                search_result: result,
                parsed_webpage: parsed_webpage.unwrap(),
            }
        })
        .collect::<Vec<_>>();
    let results = futures::future::join_all(futures).await;

    let max_concurrency = scrape_input
        .max_concurrency
        .unwrap_or(DEFAULT_MAX_CONCURRENCY);

    let default_result_format = ScrapeSiteResultFormat::default();
    let result_format = scrape_input
        .result_format
        .as_ref()
        .unwrap_or(&default_result_format);

    let formatted_results = stream::iter(results)
        .map(|result| format_result(result, result_format))
        .buffer_unordered(max_concurrency)
        .collect::<Vec<_>>()
        .await;

    let mut all_results = Vec::new();
    for formatted_result in formatted_results {
        match formatted_result {
            Ok(formatted_result) => {
                all_results.push(formatted_result);
            }
            Err(e) => {
                return Err(ScrapeSiteError::FormatError(e));
            }
        }
    }
    Ok(all_results)
}

#[derive(Error, Debug)]
pub enum ScrapeSiteFormatError {
    #[error("Failed to format result with llm: {0}")]
    LLMError(#[from] LLMError),
    #[error("Failed to parse json: {0}")]
    ParseError(#[from] ParseJsonError),
}

async fn format_result(
    result: ParsedSearchResult,
    result_format: &ScrapeSiteResultFormat,
) -> Result<ScrapeSiteResult, ScrapeSiteFormatError> {
    match result_format {
        ScrapeSiteResultFormat::Html => format_result_html(&result).await,
        ScrapeSiteResultFormat::Md => format_result_md(&result).await,
    }
}

async fn format_result_html(
    result: &ParsedSearchResult,
) -> Result<ScrapeSiteResult, ScrapeSiteFormatError> {
    Ok(ScrapeSiteResult {
        search_result: result.search_result.clone(),
        formatted_content: result.parsed_webpage.content.clone(),
    })
}

#[derive(Serialize, Deserialize, Debug, Clone)]
struct SearchResultObject {
    title: String,
    content: String,
}

async fn format_result_md(
    result: &ParsedSearchResult,
) -> Result<ScrapeSiteResult, ScrapeSiteFormatError> {
    let prompt = Prompt {
        instruction: SCRAPE_SITE_RESULT_FORMAT_MD_SYSTEM_PROMPT.to_string(),
        context: format!("# Site\n{}", result.parsed_webpage.content.clone()),
    };
    let builder = CompletionBuilder::new()
        .model("gpt-4o".to_string())
        .provider("openai".to_string())
        .messages(prompt.clone().build_messages())
        .temperature(0.0);
    let completion = match builder.build().await {
        Ok(completion) => completion,
        Err(e) => return Err(ScrapeSiteFormatError::LLMError(e)),
    };

    let search_result_object: SearchResultObject = match parse_json_response(&completion) {
        Ok(search_result_object) => search_result_object,
        Err(e) => return Err(ScrapeSiteFormatError::ParseError(e)),
    };
    let search_result = SearchResult {
        title: search_result_object.title,
        url: result.search_result.url.clone(),
        content: search_result_object.content.clone(),
    };
    Ok(ScrapeSiteResult {
        search_result,
        formatted_content: search_result_object.content,
    })
}

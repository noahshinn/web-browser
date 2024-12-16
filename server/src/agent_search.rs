use crate::llm::LLMError;
use crate::llm::{CompletionBuilder, Model, Provider};
use crate::prompts::{
    build_analyze_result_system_prompt, build_sufficient_information_check_prompt, Prompt,
    WEB_SEARCH_USE_SAME_WEB_SEARCH_FINDINGS_DOCUMENT,
};
use crate::search::SearchResult;
use crate::utils::{
    display_search_results_with_indices, enforce_n_sequential_newlines, parse_json_response,
};
use rocket::{FromForm, FromFormField};
use serde::{Deserialize, Serialize};
use std::fmt::Display;
use thiserror::Error;

use ammonia::Builder;
use reqwest;
use std::collections::HashSet;

pub mod human;
pub mod multi_query_parallel_tree;
pub mod parallel;
pub mod parallel_tree;
pub mod sequential;

pub use human::{human_agent_search, HumanAgentSearchError};
pub use multi_query_parallel_tree::{
    multi_query_parallel_tree_agent_search, MultiQueryParallelTreeAgentSearchError,
};
pub use parallel::{parallel_agent_search, ParallelAgentSearchError};
pub use parallel_tree::{parallel_tree_agent_search, ParallelTreeAgentSearchError};
pub use sequential::{sequential_agent_search, SequentialAgentSearchError};

#[derive(Deserialize, Debug, Clone, FromForm)]
pub struct SearchQuery {
    pub query: String,
    #[serde(default)]
    pub strategy: Option<AgentSearchStrategy>,
}

#[derive(Debug, Clone, Deserialize, FromFormField)]
pub enum AgentSearchStrategy {
    #[serde(rename = "human")]
    Human,
    #[serde(rename = "parallel")]
    Parallel,
    #[serde(rename = "sequential")]
    Sequential,
    #[serde(rename = "parallel_tree")]
    ParallelTree,
    #[serde(rename = "multi_query_parallel_tree")]
    MultiQueryParallelTree,
}

impl Default for AgentSearchStrategy {
    fn default() -> Self {
        AgentSearchStrategy::Human
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct AnalysisDocument {
    pub content: String,
    pub visited_results: Vec<SearchResult>,
    pub unvisited_results: Vec<SearchResult>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct AgentSearchResult {
    pub analysis: AnalysisDocument,
    pub raw_results: Vec<SearchResult>,
}

#[derive(Error, Debug)]
pub enum VisitAndExtractRelevantInfoError {
    #[error("LLM error: {0}")]
    LLMError(#[from] LLMError),
    #[error("Webpage parse failed: {0}")]
    WebpageParseError(#[from] WebpageParseError),
}

#[derive(Error, Debug)]
pub struct AggregationPassError(LLMError);

#[derive(Error, Debug)]
pub enum AgentSearchError {
    #[error("Human agent search failed: {0}")]
    HumanAgentSearchError(#[from] HumanAgentSearchError),
    #[error("Parallel agent search failed: {0}")]
    ParallelAgentSearchError(#[from] ParallelAgentSearchError),
    #[error("Sequential agent search failed: {0}")]
    SequentialAgentSearchError(#[from] SequentialAgentSearchError),
    #[error("Parallel tree agent search failed: {0}")]
    ParallelTreeAgentSearchError(#[from] ParallelTreeAgentSearchError),
    #[error("Multi query parallel tree agent search failed: {0}")]
    MultiQueryParallelTreeAgentSearchError(#[from] MultiQueryParallelTreeAgentSearchError),
}

pub async fn agent_search(
    query: &SearchQuery,
    searx_host: &str,
    searx_port: &str,
) -> Result<AgentSearchResult, AgentSearchError> {
    let strategy = query.strategy.clone().unwrap_or_default();

    match strategy {
        AgentSearchStrategy::Human => human_agent_search(&query.query, searx_host, searx_port)
            .await
            .map_err(AgentSearchError::HumanAgentSearchError),
        AgentSearchStrategy::Parallel => {
            parallel_agent_search(&query.query, searx_host, searx_port)
                .await
                .map_err(AgentSearchError::ParallelAgentSearchError)
        }
        AgentSearchStrategy::Sequential => {
            sequential_agent_search(&query.query, searx_host, searx_port)
                .await
                .map_err(AgentSearchError::SequentialAgentSearchError)
        }
        AgentSearchStrategy::ParallelTree => {
            parallel_tree_agent_search(&query.query, searx_host, searx_port)
                .await
                .map_err(AgentSearchError::ParallelTreeAgentSearchError)
        }
        AgentSearchStrategy::MultiQueryParallelTree => {
            multi_query_parallel_tree_agent_search(&query.query, searx_host, searx_port)
                .await
                .map_err(AgentSearchError::MultiQueryParallelTreeAgentSearchError)
        }
    }
}

async fn visit_and_extract_relevant_info(
    query: &str,
    current_analysis: &str,
    result: &SearchResult,
) -> Result<String, VisitAndExtractRelevantInfoError> {
    let parsed_webpage = match visit_and_parse_webpage(&result.url).await {
        Ok(parsed_webpage) => parsed_webpage,
        Err(e) => return Err(VisitAndExtractRelevantInfoError::WebpageParseError(e)),
    };
    let user_prompt = format!(
        "# Query:\n{}\n\n# Search result:\n## {} ({})\n\n{}\n\n# Current findings document:\n{}",
        query, result.title, result.url, parsed_webpage.content, current_analysis
    );
    let prompt = Prompt::new(build_analyze_result_system_prompt(), user_prompt);
    let completion = match CompletionBuilder::new()
        .model(Model::Claude35Sonnet)
        .provider(Provider::Anthropic)
        .messages(prompt.build_messages())
        .temperature(0.0)
        .build()
        .await
    {
        Ok(completion) => completion,
        Err(e) => return Err(VisitAndExtractRelevantInfoError::LLMError(e)),
    };
    if completion.contains(&WEB_SEARCH_USE_SAME_WEB_SEARCH_FINDINGS_DOCUMENT) {
        return Ok(current_analysis.to_string());
    }
    Ok(completion)
}

#[derive(Error, Debug)]
pub enum WebpageParseError {
    #[error("Failed to fetch webpage: {0}")]
    FetchError(#[from] reqwest::Error),
    #[error("Failed to parse webpage")]
    DomParseError(#[from] DomParseError),
    #[error("Failed to clean webpage: {0}")]
    SemanticParseError(#[from] SemanticParseError),
}

#[derive(Error, Debug)]
pub enum DomParseError {
    #[error("Failed to parse webpage")]
    ParseError(String),
}

#[derive(Error, Debug)]
pub enum SemanticParseError {
    #[error("Failed to parse webpage content: {0}")]
    ParseError(String),
}

pub struct ParsedWebpage {
    pub original_content: String,
    pub content: String,
}

pub async fn visit_and_parse_webpage(url: &str) -> Result<ParsedWebpage, WebpageParseError> {
    let response = match reqwest::get(url).await {
        Ok(response) => response,
        Err(e) => return Err(WebpageParseError::FetchError(e)),
    };
    let webpage_text = response
        .text()
        .await
        .map_err(|e| WebpageParseError::FetchError(e))?;

    let dom_text = dom_parse_webpage(&webpage_text)?;
    // let semantic_text = semantic_parse_webpage(&dom_text).await?;
    // trim the leading and trailing whitespace
    let trimmed_text = dom_text.content.trim();
    Ok(ParsedWebpage {
        original_content: dom_text.original_content,
        content: trimmed_text.to_string(),
    })
}

const WHITELISTED_ATTRIBUTES: [&str; 10] = [
    "data-label",
    "href",
    "label",
    "alt",
    "title",
    "aria-label",
    "aria-description",
    "role",
    "type",
    "name",
];
const BLACKLISTED_TAGS: [&str; 27] = [
    "abbr",
    "script",
    "style",
    "noscript",
    "iframe",
    "svg",
    "span",
    "cite",
    "i",
    "b",
    "u",
    "em",
    "strong",
    "small",
    "s",
    "q",
    "figcaption",
    "figure",
    "footer",
    "header",
    "nav",
    "section",
    "article",
    "aside",
    "main",
    "canvas",
    "center",
];

fn dom_parse_webpage(webpage_text: &str) -> Result<ParsedWebpage, DomParseError> {
    let clean_html = Builder::new()
        .rm_tags(BLACKLISTED_TAGS)
        .generic_attributes(HashSet::from_iter(WHITELISTED_ATTRIBUTES))
        .attribute_filter(|element, attribute, value| match (element, attribute) {
            ("div", "src") => None,
            ("img", "src") => None,
            ("img", "height") => None,
            ("img", "width") => None,
            ("a", "rel") => None,
            _ => Some(value.into()),
        })
        .strip_comments(true)
        .clean(&webpage_text)
        .to_string();
    let clean_html = clean_html
        .lines()
        .filter(|line| !line.trim().is_empty())
        .collect::<Vec<&str>>()
        .join("\n");
    let clean_html = enforce_n_sequential_newlines(&clean_html, 2);
    Ok(ParsedWebpage {
        original_content: webpage_text.to_string(),
        content: clean_html,
    })
}

#[derive(Deserialize, Debug, Clone)]
struct SufficientInformationCheck {
    sufficient: bool,
}

#[derive(Error, Debug)]
pub struct SufficientInformationCheckError(LLMError);

impl Display for SufficientInformationCheckError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Sufficient information check failed: {}", self.0)
    }
}

async fn check_sufficient_information(
    query: &str,
    current_analysis: &str,
    visited_results: &[SearchResult],
    unvisited_results: &[SearchResult],
) -> Result<SufficientInformationCheck, SufficientInformationCheckError> {
    let user_prompt = format!("# Query:\n{}\n\n# Current analysis:\n{}\n\n# Visited results:\n{}\n\n# Unvisited results:\n{}", query, current_analysis, display_search_results_with_indices(visited_results), display_search_results_with_indices(unvisited_results));
    let prompt = Prompt::new(build_sufficient_information_check_prompt(), user_prompt);
    let completion = match CompletionBuilder::new()
        .model(Model::Claude35Sonnet)
        .provider(Provider::Anthropic)
        .messages(prompt.build_messages())
        .temperature(0.0)
        .build()
        .await
    {
        Ok(completion) => completion,
        Err(e) => return Err(SufficientInformationCheckError(e)),
    };
    let decision: SufficientInformationCheck = match parse_json_response(&completion) {
        Ok(decision) => decision,
        Err(e) => {
            return Err(SufficientInformationCheckError(LLMError::ParseError(
                e.to_string(),
            )))
        }
    };
    Ok(decision)
}

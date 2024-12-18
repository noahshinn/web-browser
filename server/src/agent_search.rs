use crate::llm::LLMError;
use crate::llm::{CompletionBuilder, Model, Provider};
use crate::prompts::{
    build_analyze_result_system_prompt, build_sufficient_information_check_prompt, Prompt,
    AGGREGATE_WEB_SEARCH_FINDINGS_PROMPT, WEB_SEARCH_USE_SAME_WEB_SEARCH_FINDINGS_DOCUMENT,
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
use futures::future::join_all;
use reqwest;
use std::collections::HashSet;
use tokio::task;
use tokio::task::JoinError;

pub mod human;
pub mod parallel;
pub mod parallel_tree;
pub mod sequential;

pub use human::{human_agent_search, HumanAgentSearchError};
pub use parallel::{parallel_agent_search, ParallelAgentSearchError};
pub use parallel_tree::{parallel_tree_agent_search, ParallelTreeAgentSearchError};
pub use sequential::{sequential_agent_search, SequentialAgentSearchError};

#[derive(Deserialize, Debug, Clone, FromForm)]
pub struct SearchInput {
    pub query: String,
    #[serde(default)]
    pub strategy: Option<AgentSearchStrategy>,
    #[serde(default)]
    pub max_results_to_visit: Option<usize>,
}

impl Default for SearchInput {
    fn default() -> Self {
        Self {
            query: String::new(),
            strategy: Some(AgentSearchStrategy::Human),
            max_results_to_visit: Some(10),
        }
    }
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
    #[error("Join error: {0}")]
    JoinError(#[from] JoinError),
}

#[derive(Error, Debug)]
pub struct AggregationPassError(LLMError);

impl Display for AggregationPassError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Aggregation pass failed: {}", self.0)
    }
}

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
}

pub async fn agent_search(
    search_input: &SearchInput,
    searx_host: &str,
    searx_port: &str,
) -> Result<AgentSearchResult, AgentSearchError> {
    let strategy = search_input.strategy.clone().unwrap_or_default();
    match strategy {
        AgentSearchStrategy::Human => human_agent_search(&search_input, searx_host, searx_port)
            .await
            .map_err(AgentSearchError::HumanAgentSearchError),
        AgentSearchStrategy::Parallel => {
            parallel_agent_search(&search_input, searx_host, searx_port)
                .await
                .map_err(AgentSearchError::ParallelAgentSearchError)
        }
        AgentSearchStrategy::Sequential => {
            sequential_agent_search(&search_input, searx_host, searx_port)
                .await
                .map_err(AgentSearchError::SequentialAgentSearchError)
        }
        AgentSearchStrategy::ParallelTree => {
            parallel_tree_agent_search(&search_input, searx_host, searx_port)
                .await
                .map_err(AgentSearchError::ParallelTreeAgentSearchError)
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

#[derive(Deserialize, Debug, Clone)]
struct ExtractionResult {
    search_result: SearchResult,
    content: String,
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

pub async fn parallel_visit_and_extract_relevant_info(
    query: &str,
    search_results: &[SearchResult],
    current_analysis: &str,
) -> Result<AgentSearchResult, ParallelAgentSearchError> {
    let extraction_tasks = search_results
        .iter()
        .map(|result| {
            let query = query.to_string();
            let current_analysis = current_analysis.to_string();
            let result = result.clone();
            task::spawn(async move {
                visit_and_extract_relevant_info(query.as_str(), &current_analysis, &result).await
            })
        })
        .collect::<Vec<_>>();
    let extraction_results: Vec<ExtractionResult> = join_all(extraction_tasks)
        .await
        .into_iter()
        .collect::<Result<Vec<_>, _>>()?
        .into_iter()
        .enumerate()
        .map(|(index, result)| {
            result
                .map(|content| ExtractionResult {
                    search_result: search_results[index].clone(),
                    content,
                })
                .map_err(ParallelAgentSearchError::VisitAndExtractRelevantInfoError)
        })
        .collect::<Result<Vec<_>, _>>()?;
    let aggregated_result = match aggregate_results(query, extraction_results).await {
        Ok(result) => AgentSearchResult {
            analysis: AnalysisDocument {
                content: result,
                visited_results: search_results.to_vec(),
                unvisited_results: Vec::new(),
            },
            raw_results: search_results.to_vec(),
        },
        Err(e) => return Err(ParallelAgentSearchError::AggregationPassError(e)),
    };
    Ok(aggregated_result)
}

async fn aggregate_results(
    query: &str,
    extraction_results: Vec<ExtractionResult>,
) -> Result<String, AggregationPassError> {
    let extraction_results_display = extraction_results
        .iter()
        .map(|result| {
            format!(
                "## {} ({})\n\n{}",
                result.search_result.title, result.search_result.url, result.content
            )
        })
        .collect::<Vec<_>>()
        .join("\n\n");
    let user_prompt = format!(
        r#"# Search query
{query}

# Extracted information
{extraction_results_display}"#
    );
    let prompt = Prompt::new(
        AGGREGATE_WEB_SEARCH_FINDINGS_PROMPT.to_string(),
        user_prompt,
    );
    let completion = match CompletionBuilder::new()
        .model(Model::Claude35Sonnet)
        .provider(Provider::Anthropic)
        .messages(prompt.build_messages())
        .temperature(0.0)
        .build()
        .await
    {
        Ok(completion) => completion,
        Err(e) => return Err(AggregationPassError(e)),
    };
    Ok(completion)
}

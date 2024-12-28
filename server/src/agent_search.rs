use crate::llm::default_completion;
use crate::llm::LLMError;
use crate::prompts::{
    build_analyze_result_system_prompt, build_sufficient_information_check_prompt, Prompt,
    AGGREGATE_WEB_SEARCH_FINDINGS_PROMPT, WEB_SEARCH_USE_SAME_WEB_SEARCH_FINDINGS_DOCUMENT,
};
use crate::query::QueryStrategy;
use crate::result_format::{
    format_result, AnalysisDocument, ResultFormat, ResultFormatError, ResultFormatResponse,
};
use crate::search::SearchResult;
use crate::utils::{display_search_results_with_indices, parse_json_response};
use crate::webpage_parse::{visit_and_parse_webpage, WebpageParseError};
use rocket::{FromForm, FromFormField};
use serde::{Deserialize, Serialize};
use std::fmt::Display;
use thiserror::Error;

use futures::future::join_all;
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

use crate::query::{synthesize_queries, QuerySynthesisError};

#[derive(Deserialize, Debug, Clone, FromForm)]
pub struct AgentSearchInput {
    pub query: String,
    pub current_search_result: Option<SearchResult>,
    #[serde(default)]
    pub search_strategy: Option<AgentSearchStrategy>,
    #[serde(default)]
    pub query_strategy: Option<QueryStrategy>,
    #[serde(default)]
    pub max_results_to_visit: Option<usize>,
    #[serde(default)]
    pub result_format: Option<ResultFormat>,
    #[serde(default)]
    pub custom_result_format_description: Option<String>,
    #[serde(default)]
    pub whitelisted_base_urls: Option<Vec<String>>,
    #[serde(default)]
    pub blacklisted_base_urls: Option<Vec<String>>,
}

impl Default for AgentSearchInput {
    fn default() -> Self {
        Self {
            query: String::new(),
            current_search_result: None,
            search_strategy: Some(AgentSearchStrategy::default()),
            query_strategy: Some(QueryStrategy::default()),
            max_results_to_visit: Some(10),
            result_format: Some(ResultFormat::default()),
            custom_result_format_description: None,
            whitelisted_base_urls: None,
            blacklisted_base_urls: None,
        }
    }
}

impl AgentSearchInput {
    pub fn build_google_search_query(&self) -> String {
        crate::search::build_google_search_query(
            &self.query,
            self.whitelisted_base_urls.as_ref(),
            self.blacklisted_base_urls.as_ref(),
        )
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
pub struct AgentSearchResult {
    pub raw_analysis: AnalysisDocument,
    pub queries_executed: Vec<String>,
    pub response: ResultFormatResponse,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PreFormattedAgentSearchResult {
    pub raw_analysis: AnalysisDocument,
    pub queries_executed: Vec<String>,
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
pub enum AgentSingleSearchError {
    #[error("Human agent search failed: {0}")]
    HumanAgentSearchError(#[from] HumanAgentSearchError),
    #[error("Parallel agent search failed: {0}")]
    ParallelAgentSearchError(#[from] ParallelAgentSearchError),
    #[error("Sequential agent search failed: {0}")]
    SequentialAgentSearchError(#[from] SequentialAgentSearchError),
    #[error("Parallel tree agent search failed: {0}")]
    ParallelTreeAgentSearchError(#[from] ParallelTreeAgentSearchError),
}

#[derive(Error, Debug)]
pub enum AgentSearchError {
    #[error("Query synthesis failed: {0}")]
    QuerySynthesisError(#[from] QuerySynthesisError),
    #[error("Agent single search failed: {0}")]
    SingleSearchError(#[from] AgentSingleSearchError),
    #[error("Result format failed: {0}")]
    ResultFormatError(#[from] ResultFormatError),
}

pub async fn agent_search_with_query(
    search_input: &AgentSearchInput,
    searx_host: &str,
    searx_port: &str,
) -> Result<PreFormattedAgentSearchResult, AgentSingleSearchError> {
    let search_strategy = search_input.search_strategy.clone().unwrap_or_default();
    match search_strategy {
        AgentSearchStrategy::Human => human_agent_search(&search_input, searx_host, searx_port)
            .await
            .map_err(AgentSingleSearchError::HumanAgentSearchError),
        AgentSearchStrategy::Parallel => {
            parallel_agent_search(&search_input, searx_host, searx_port)
                .await
                .map_err(AgentSingleSearchError::ParallelAgentSearchError)
        }
        AgentSearchStrategy::Sequential => {
            sequential_agent_search(&search_input, searx_host, searx_port)
                .await
                .map_err(AgentSingleSearchError::SequentialAgentSearchError)
        }
        AgentSearchStrategy::ParallelTree => {
            parallel_tree_agent_search(&search_input, searx_host, searx_port)
                .await
                .map_err(AgentSingleSearchError::ParallelTreeAgentSearchError)
        }
    }
}

pub async fn agent_search(
    search_input: &AgentSearchInput,
    searx_host: &str,
    searx_port: &str,
) -> Result<AgentSearchResult, AgentSearchError> {
    let query_strategy = search_input.query_strategy.clone().unwrap_or_default();
    let search_strategy = search_input.search_strategy.clone().unwrap_or_default();
    let synthesized_queries = synthesize_queries(&search_input.query, &query_strategy)
        .await
        .map_err(|e| AgentSearchError::QuerySynthesisError(e))?;
    let current_search_result: Option<SearchResult> = search_input.current_search_result.clone();
    let pre_formatted_result: PreFormattedAgentSearchResult = match query_strategy {
        QueryStrategy::Verbatim | QueryStrategy::Single => {
            let query = synthesized_queries.queries.first().unwrap();
            let modified_input = AgentSearchInput {
                query: query.clone(),
                current_search_result: current_search_result.clone(),
                search_strategy: Some(search_strategy.clone()),
                query_strategy: None,
                max_results_to_visit: search_input.max_results_to_visit,
                result_format: search_input.result_format.clone(),
                custom_result_format_description: search_input
                    .custom_result_format_description
                    .clone(),
                whitelisted_base_urls: search_input.whitelisted_base_urls.clone(),
                blacklisted_base_urls: search_input.blacklisted_base_urls.clone(),
            };
            let pre_formatted_result =
                match agent_search_with_query(&modified_input, searx_host, searx_port).await {
                    Ok(result) => result,
                    Err(e) => return Err(AgentSearchError::SingleSearchError(e)),
                };
            pre_formatted_result
        }
        QueryStrategy::Sequential => {
            let mut cur_analysis = AnalysisDocument {
                content: String::new(),
                visited_results: Vec::new(),
                unvisited_results: Vec::new(),
            };
            let mut queries_executed = Vec::new();

            for query in synthesized_queries.queries {
                let modified_input = AgentSearchInput {
                    query: query.clone(),
                    current_search_result: current_search_result.clone(),
                    search_strategy: Some(search_strategy.clone()),
                    query_strategy: None,
                    max_results_to_visit: search_input.max_results_to_visit,
                    result_format: search_input.result_format.clone(),
                    custom_result_format_description: search_input
                        .custom_result_format_description
                        .clone(),
                    whitelisted_base_urls: search_input.whitelisted_base_urls.clone(),
                    blacklisted_base_urls: search_input.blacklisted_base_urls.clone(),
                };
                let iter_result =
                    match agent_search_with_query(&modified_input, searx_host, searx_port).await {
                        Ok(result) => result,
                        Err(e) => return Err(AgentSearchError::SingleSearchError(e)),
                    };

                if cur_analysis.content.is_empty() {
                    cur_analysis = iter_result.raw_analysis;
                } else {
                    cur_analysis = AnalysisDocument {
                        content: format!(
                            "{}\n\n{}",
                            cur_analysis.content, iter_result.raw_analysis.content
                        ),
                        visited_results: cur_analysis
                            .visited_results
                            .into_iter()
                            .chain(iter_result.raw_analysis.visited_results.into_iter())
                            .collect(),
                        unvisited_results: cur_analysis
                            .unvisited_results
                            .into_iter()
                            .chain(iter_result.raw_analysis.unvisited_results.into_iter())
                            .collect(),
                    };
                }
                queries_executed.extend(iter_result.queries_executed);
            }
            PreFormattedAgentSearchResult {
                raw_analysis: cur_analysis,
                queries_executed,
            }
        }
        QueryStrategy::Parallel => {
            let tasks = synthesized_queries.queries.iter().map(|query| {
                let query = query.clone();
                let current_search_result = current_search_result.clone();
                let search_strategy = search_strategy.clone();
                let max_results_to_visit = search_input.max_results_to_visit;
                let result_format = search_input.result_format.clone();
                let searx_host = searx_host.to_string();
                let searx_port = searx_port.to_string();
                let custom_result_format_description =
                    search_input.custom_result_format_description.clone();
                let whitelisted_base_urls = search_input.whitelisted_base_urls.clone();
                let blacklisted_base_urls = search_input.blacklisted_base_urls.clone();
                tokio::spawn(async move {
                    let modified_input = AgentSearchInput {
                        query,
                        current_search_result,
                        search_strategy: Some(search_strategy),
                        query_strategy: None,
                        max_results_to_visit,
                        result_format,
                        custom_result_format_description,
                        whitelisted_base_urls,
                        blacklisted_base_urls,
                    };
                    agent_search_with_query(&modified_input, &searx_host, &searx_port).await
                })
            });
            let join_results = futures::future::join_all(tasks).await;
            let mut results = Vec::new();
            for join_result in join_results {
                match join_result {
                    Ok(search_result) => match search_result {
                        Ok(result) => results.push(result),
                        Err(e) => return Err(AgentSearchError::SingleSearchError(e)),
                    },
                    Err(e) => {
                        return Err(AgentSearchError::SingleSearchError(
                            AgentSingleSearchError::ParallelAgentSearchError(
                                ParallelAgentSearchError::JoinError(e),
                            ),
                        ))
                    }
                }
            }
            let mut cur_analysis = AnalysisDocument {
                content: String::new(),
                visited_results: Vec::new(),
                unvisited_results: Vec::new(),
            };
            let mut queries_executed = Vec::new();
            for res in results {
                if cur_analysis.content.is_empty() {
                    cur_analysis = res.raw_analysis;
                } else {
                    cur_analysis = AnalysisDocument {
                        content: format!(
                            "{}\n\n{}",
                            cur_analysis.content, res.raw_analysis.content
                        ),
                        visited_results: cur_analysis
                            .visited_results
                            .clone()
                            .into_iter()
                            .chain(res.raw_analysis.visited_results.clone().into_iter())
                            .collect(),
                        unvisited_results: cur_analysis
                            .unvisited_results
                            .clone()
                            .into_iter()
                            .chain(res.raw_analysis.unvisited_results.clone().into_iter())
                            .collect(),
                    };
                }
                queries_executed.extend(res.queries_executed);
            }
            PreFormattedAgentSearchResult {
                raw_analysis: cur_analysis,
                queries_executed,
            }
        }
    };
    let result_format = search_input.result_format.clone().unwrap_or_default();
    let response = match format_result(
        &search_input.query,
        &pre_formatted_result.raw_analysis,
        &result_format,
        search_input.custom_result_format_description.as_deref(),
    )
    .await
    {
        Ok(response) => response,
        Err(e) => return Err(AgentSearchError::ResultFormatError(e)),
    };
    Ok(AgentSearchResult {
        raw_analysis: pre_formatted_result.raw_analysis,
        queries_executed: pre_formatted_result.queries_executed,
        response,
    })
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
    let completion = match default_completion(&prompt).await {
        Ok(completion) => completion,
        Err(e) => return Err(VisitAndExtractRelevantInfoError::LLMError(e)),
    };
    if completion.contains(&WEB_SEARCH_USE_SAME_WEB_SEARCH_FINDINGS_DOCUMENT) {
        return Ok(current_analysis.to_string());
    }
    Ok(completion)
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
    let completion = match default_completion(&prompt).await {
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
) -> Result<PreFormattedAgentSearchResult, ParallelAgentSearchError> {
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
        Ok(result) => PreFormattedAgentSearchResult {
            raw_analysis: AnalysisDocument {
                content: result,
                visited_results: search_results.to_vec(),
                unvisited_results: Vec::new(),
            },
            queries_executed: vec![query.to_string()],
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
    let completion = match default_completion(&prompt).await {
        Ok(completion) => completion,
        Err(e) => return Err(AggregationPassError(e)),
    };
    Ok(completion)
}

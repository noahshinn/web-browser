use crate::search::{SearchResult, search, SearchError};
use crate::prompts::{build_analyze_result_system_prompt, build_select_next_result_system_prompt, build_sufficient_findings_document_prompt, WEB_SEARCH_USE_SAME_WEB_SEARCH_FINDINGS_DOCUMENT};
use crate::llm::{CompletionBuilder, Message, Model, Provider, Role, LLMError};
use crate::utils::{display_search_results_with_indices, parse_json_response};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use std::fmt::Display;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct AnalysisDocument {
    content: String,
    used_results: Vec<SearchResult>,
    discarded_results: Vec<SearchResult>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct AgentSearchResult {
    analysis: AnalysisDocument,
    raw_results: Vec<SearchResult>,
}

#[derive(Error, Debug)]
pub struct SearchResultAnalysisError(LLMError);

impl Display for SearchResultAnalysisError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Search result analysis failed: {}", self.0)
    }
}

#[derive(Error, Debug)]
pub struct InsufficientFindingsCheckError(LLMError);

impl Display for InsufficientFindingsCheckError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Insufficient findings check failed: {}", self.0)
    }
}

#[derive(Error, Debug)]
pub struct SelectNextResultError(LLMError);

impl Display for SelectNextResultError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Failed to select next result: {}", self.0)
    }
}

#[derive(Error, Debug)]
pub enum AgentSearchError {
    #[error("Search failed: {0}")]
    SearchError(#[from] SearchError),
    #[error("Analysis failed: {0}")]
    AnalysisError(#[from] SearchResultAnalysisError),
    #[error("Insufficient findings check failed: {0}")]
    InsufficientFindingsCheckError(#[from] InsufficientFindingsCheckError),
    #[error("Failed to select next result: {0}")]
    SelectNextResultError(#[from] SelectNextResultError),
}

#[derive(Deserialize, Debug, Clone)]
struct LLMDecision {
    keep_current: bool,
    new_analysis: Option<String>,
}

#[derive(Deserialize, Debug, Clone)]
struct NextResultToVisit {
    index: usize,
}

#[derive(Deserialize, Debug, Clone)]
struct SufficientFindingsCheck {
    sufficient: bool,
}



async fn analyze_result(
    query: &str,
    current_analysis: &str,
    result: &SearchResult,
) -> Result<LLMDecision, Box<dyn std::error::Error>> {
    let prompt = vec![
        Message {
            role: Role::System,
            content: build_analyze_result_system_prompt(),
        },
        Message {
            role: Role::User,
            content: format!("# Query:\n{}\n\n# Search result:\n{}\n\n# Current findings document:\n{}", query, result, current_analysis),
        },
    ];
    let completion = match CompletionBuilder::new()
        .model(Model::Claude35Sonnet)
        .provider(Provider::Anthropic)
        .messages(prompt)
        .temperature(0.0)
        .build()
        .await
    {
        Ok(completion) => completion,
        Err(e) => return Err(Box::new(e)),
    };

    if completion.contains(&WEB_SEARCH_USE_SAME_WEB_SEARCH_FINDINGS_DOCUMENT) {
        return Ok(LLMDecision {
            keep_current: true,
            new_analysis: None,
        });
    }
    Ok(LLMDecision {
        keep_current: false,
        new_analysis: Some(completion),
    })
}

async fn select_next_result(
    query: &str,
    current_analysis: &str,
    visited_results: &[SearchResult],
    unvisited_results: &[SearchResult],
) -> Result<usize, AgentSearchError> {
    let prompt = vec![
        Message {
            role: Role::System,
            content: build_select_next_result_system_prompt(),
        },
        Message {
            role: Role::User,
            content: format!("# Query:\n{}\n\n# Current analysis:\n{}\n\n# Visited results:\n{}\n\n# Unvisited results:\n{}", query, current_analysis, display_search_results_with_indices(visited_results), display_search_results_with_indices(unvisited_results)),
        },
    ];
    let completion = match CompletionBuilder::new()
        .model(Model::Claude35Sonnet)
        .provider(Provider::Anthropic)
        .messages(prompt)
        .temperature(0.0)
        .build()
        .await
    {
        Ok(completion) => completion,
        Err(e) => return Err(AgentSearchError::SelectNextResultError(SelectNextResultError(e))),
    };

    let decision: NextResultToVisit = match parse_json_response(&completion) {
        Ok(decision) => decision,
        Err(e) => return Err(AgentSearchError::SelectNextResultError(SelectNextResultError(LLMError::ParseError(e.to_string())))),
    };
    Ok(decision.index)
}

async fn check_sufficient_findings_document(
    query: &str,
    current_analysis: &str,
    used_results: &[SearchResult],
) -> Result<SufficientFindingsCheck, Box<dyn std::error::Error>> {
    let prompt = vec![
        Message {
            role: Role::System,
            content: build_sufficient_findings_document_prompt(),
        },
        Message {
            role: Role::User,
            content: format!("# Query:\n{}\n\n# Current analysis:\n{}\n\n# Used results:\n{}", query, current_analysis, display_search_results_with_indices(used_results)),
        },
    ];
    let completion = match CompletionBuilder::new()
        .model(Model::Claude35Sonnet)
        .provider(Provider::Anthropic)
        .messages(prompt)
        .temperature(0.0)
        .build()
        .await
    {
        Ok(completion) => completion,
        Err(e) => return Err(Box::new(e)),
    };

    let decision: SufficientFindingsCheck = match parse_json_response(&completion) {
        Ok(decision) => decision,
        Err(e) => return Err(Box::new(e)),
    };
    Ok(decision)
}

pub async fn agent_search(query: &str, searx_host: &str, searx_port: &str) -> Result<AgentSearchResult, AgentSearchError> {
    let search_result = match search(query, searx_host, searx_port).await {
        Ok(results) => results,
        Err(e) => return Err(AgentSearchError::SearchError(e)),
    };

    let mut analysis = AnalysisDocument {
        content: String::new(),
        used_results: Vec::new(),
        discarded_results: Vec::new(),
    };
    let mut unvisited_results = search_result.clone();

    while !unvisited_results.is_empty() {
        let next_index = match select_next_result(query, &analysis.content, &analysis.used_results, &unvisited_results).await {
            Ok(idx) => idx,
            Err(e) => return Err(e),
        };

        let result = unvisited_results.remove(next_index);
        match analyze_result(query, &analysis.content, &result).await {
            Ok(decision) => {
                if decision.keep_current {
                    analysis.discarded_results.push(result);
                } else if let Some(new_analysis) = decision.new_analysis {
                    analysis.content = new_analysis;
                    analysis.used_results.push(result);
                }
            }
            Err(e) => return Err(AgentSearchError::AnalysisError(SearchResultAnalysisError(LLMError::ParseError(format!("Failed to parse JSON: {}", e))))),
        }

        let sufficient = match check_sufficient_findings_document(query, &analysis.content, &analysis.used_results).await {
            Ok(decision) => decision.sufficient,
            Err(e) => return Err(AgentSearchError::InsufficientFindingsCheckError(InsufficientFindingsCheckError(LLMError::ParseError(format!("Failed to parse JSON: {}", e))))),
        };

        if sufficient {
            break;
        }
    }

    Ok(AgentSearchResult {
        analysis: analysis,
        raw_results: search_result,
    })
}
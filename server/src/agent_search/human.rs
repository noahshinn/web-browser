use serde::Deserialize;
use std::fmt::Display;
use thiserror::Error;

use crate::agent_search::{
    check_sufficient_information, visit_and_extract_relevant_info, AgentSearchInput,
    AnalysisDocument, LLMError, PreFormattedAgentSearchResult, SearchResult,
    SufficientInformationCheckError, VisitAndExtractRelevantInfoError,
};
use crate::llm::{CompletionBuilder, Model, Provider};
use crate::prompts::{build_select_next_result_system_prompt, Prompt};
use crate::search;
use crate::search::{search, SearchError};
use crate::utils::{display_search_results_with_indices, parse_json_response};

#[derive(Error, Debug)]
pub struct SelectNextResultError(LLMError);

impl Display for SelectNextResultError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Failed to select next result: {}", self.0)
    }
}

#[derive(Error, Debug)]
pub enum HumanAgentSearchError {
    #[error("Search failed: {0}")]
    SearchError(#[from] SearchError),
    #[error("Visit and extract relevant info failed: {0}")]
    VisitAndExtractRelevantInfoError(#[from] VisitAndExtractRelevantInfoError),
    #[error("Sufficient information check failed: {0}")]
    SufficientInformationCheckError(#[from] SufficientInformationCheckError),
    #[error("Failed to select next result: {0}")]
    SelectNextResultError(#[from] SelectNextResultError),
}

#[derive(Deserialize, Debug, Clone)]
struct NextResultToVisit {
    index: usize,
}

async fn select_next_result(
    query: &str,
    current_analysis: &str,
    visited_results: &[SearchResult],
    unvisited_results: &[SearchResult],
) -> Result<usize, SelectNextResultError> {
    let user_prompt = format!("# Query:\n{}\n\n# Current analysis:\n{}\n\n# Visited results:\n{}\n\n# Unvisited results:\n{}", query, current_analysis, display_search_results_with_indices(visited_results), display_search_results_with_indices(unvisited_results));
    let prompt = Prompt::new(build_select_next_result_system_prompt(), user_prompt);
    let completion = match CompletionBuilder::new()
        .model(Model::Claude35Sonnet)
        .provider(Provider::Anthropic)
        .messages(prompt.build_messages())
        .temperature(0.0)
        .build()
        .await
    {
        Ok(completion) => completion,
        Err(e) => return Err(SelectNextResultError(e)),
    };

    let decision: NextResultToVisit = match parse_json_response(&completion) {
        Ok(decision) => decision,
        Err(e) => return Err(SelectNextResultError(LLMError::ParseError(e.to_string()))),
    };
    Ok(decision.index)
}

pub async fn human_agent_search(
    search_input: &AgentSearchInput,
    searx_host: &str,
    searx_port: &str,
) -> Result<PreFormattedAgentSearchResult, HumanAgentSearchError> {
    let search_result = match search(
        &search::SearchInput {
            query: search_input.build_google_search_query(),
            max_results_to_visit: search_input.max_results_to_visit,
            whitelisted_base_urls: search_input.whitelisted_base_urls.clone(),
            blacklisted_base_urls: search_input.blacklisted_base_urls.clone(),
        },
        searx_host,
        searx_port,
    )
    .await
    {
        Ok(results) => results,
        Err(e) => return Err(HumanAgentSearchError::SearchError(e)),
    };
    let mut analysis = AnalysisDocument {
        content: String::new(),
        visited_results: Vec::new(),
        unvisited_results: Vec::new(),
    };
    let mut unvisited_results = search_result.clone();
    while !unvisited_results.is_empty() {
        let next_index = match select_next_result(
            &search_input.query,
            &analysis.content,
            &analysis.visited_results,
            &unvisited_results,
        )
        .await
        {
            Ok(idx) => idx,
            Err(e) => return Err(HumanAgentSearchError::SelectNextResultError(e)),
        };
        let result = unvisited_results.remove(next_index);
        match visit_and_extract_relevant_info(&search_input.query, &analysis.content, &result).await
        {
            Ok(new_analysis) => {
                analysis.content = new_analysis;
                analysis.unvisited_results.push(result);
            }
            Err(e) => return Err(HumanAgentSearchError::VisitAndExtractRelevantInfoError(e)),
        }
        match check_sufficient_information(
            &search_input.query,
            &analysis.content,
            &analysis.visited_results,
            &analysis.unvisited_results,
        )
        .await
        {
            Ok(decision) => {
                if decision.sufficient {
                    break;
                }
            }
            Err(e) => return Err(HumanAgentSearchError::SufficientInformationCheckError(e)),
        }
    }
    Ok(PreFormattedAgentSearchResult {
        raw_analysis: analysis,
        queries_executed: vec![search_input.query.clone()],
    })
}

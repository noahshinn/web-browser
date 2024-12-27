use crate::agent_search::{
    parallel_visit_and_extract_relevant_info, AgentSearchInput, AggregationPassError,
    PreFormattedAgentSearchResult, VisitAndExtractRelevantInfoError,
};
use crate::search;
use crate::search::{search, SearchError};
use thiserror::Error;
use tokio::task::JoinError;

#[derive(Error, Debug)]
pub enum ParallelAgentSearchError {
    #[error("Search failed: {0}")]
    SearchError(#[from] SearchError),
    #[error("Visit and extract relevant info failed: {0}")]
    VisitAndExtractRelevantInfoError(#[from] VisitAndExtractRelevantInfoError),
    #[error("Aggregation pass failed: {0}")]
    AggregationPassError(#[from] AggregationPassError),
    #[error("Join error: {0}")]
    JoinError(#[from] JoinError),
}

pub async fn parallel_agent_search(
    search_input: &AgentSearchInput,
    searx_host: &str,
    searx_port: &str,
) -> Result<PreFormattedAgentSearchResult, ParallelAgentSearchError> {
    let search_results = match search(
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
        Err(e) => return Err(ParallelAgentSearchError::SearchError(e)),
    };
    parallel_visit_and_extract_relevant_info(&search_input.query, &search_results, "").await
}

use thiserror::Error;
use crate::agent_search::AgentSearchResult;
use crate::search::SearchError;

#[derive(Error, Debug)]
pub enum ParallelAgentSearchError {
    #[error("Search failed: {0}")]
    SearchError(#[from] SearchError),
}

pub async fn parallel_agent_search(
    _query: &str,
    _searx_host: &str,
    _searx_port: &str,
) -> Result<AgentSearchResult, ParallelAgentSearchError> {
    todo!("Implement parallel agent search")
}

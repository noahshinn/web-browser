use thiserror::Error;
use crate::agent_search::AgentSearchResult;
use crate::search::SearchError;

#[derive(Error, Debug)]
pub enum SequentialAgentSearchError {
    #[error("Search failed: {0}")]
    SearchError(#[from] SearchError),
}

pub async fn sequential_agent_search(
    _query: &str,
    _searx_host: &str,
    _searx_port: &str,
) -> Result<AgentSearchResult, SequentialAgentSearchError> {
    todo!("Implement sequential agent search")
}

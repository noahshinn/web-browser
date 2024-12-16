use crate::agent_search::AgentSearchResult;
use crate::search::SearchError;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum ParallelTreeAgentSearchError {
    #[error("Search failed: {0}")]
    SearchError(#[from] SearchError),
}

pub async fn parallel_tree_agent_search(
    _query: &str,
    _searx_host: &str,
    _searx_port: &str,
) -> Result<AgentSearchResult, ParallelTreeAgentSearchError> {
    todo!("Implement parallel tree agent search")
}

use crate::agent_search::AgentSearchResult;
use crate::search::SearchError;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum MultiQueryParallelTreeAgentSearchError {
    #[error("Search failed: {0}")]
    SearchError(#[from] SearchError),
}

pub async fn multi_query_parallel_tree_agent_search(
    _query: &str,
    _searx_host: &str,
    _searx_port: &str,
) -> Result<AgentSearchResult, MultiQueryParallelTreeAgentSearchError> {
    todo!("Implement multi query parallel tree agent search")
}

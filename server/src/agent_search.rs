use crate::search::SearchResult;
use crate::llm::LLMError;
use crate::agent_search::utils::WebpageParseError;
use rocket::{FromForm, FromFormField};
use serde::{Deserialize, Serialize};
use thiserror::Error;

pub mod human;
pub mod parallel;
pub mod sequential;
pub mod parallel_tree;
pub mod multi_query_parallel_tree;
pub mod utils;

pub use human::{human_agent_search, HumanAgentSearchError};
pub use parallel::{parallel_agent_search, ParallelAgentSearchError};
pub use sequential::{sequential_agent_search, SequentialAgentSearchError};
pub use parallel_tree::{parallel_tree_agent_search, ParallelTreeAgentSearchError};
pub use multi_query_parallel_tree::{multi_query_parallel_tree_agent_search, MultiQueryParallelTreeAgentSearchError};

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
    pub used_results: Vec<SearchResult>,
    pub discarded_results: Vec<SearchResult>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct AgentSearchResult {
    pub analysis: AnalysisDocument,
    pub raw_results: Vec<SearchResult>,
}

#[derive(Error, Debug)]
pub enum SearchResultAnalysisError {
    #[error("Search result analysis failed: {0}")]
    LLMError(#[from] LLMError),
    #[error("Webpage parse failed: {0}")]
    WebpageParseError(#[from] WebpageParseError),
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
        AgentSearchStrategy::Human => human_agent_search(
            &query.query, 
            searx_host, 
            searx_port
        ).await.map_err(AgentSearchError::HumanAgentSearchError),
        AgentSearchStrategy::Parallel => parallel_agent_search(
            &query.query,
            searx_host,
            searx_port
        ).await.map_err(AgentSearchError::ParallelAgentSearchError),
        AgentSearchStrategy::Sequential => sequential_agent_search(
            &query.query,
            searx_host,
            searx_port
        ).await.map_err(AgentSearchError::SequentialAgentSearchError),
        AgentSearchStrategy::ParallelTree => parallel_tree_agent_search(
            &query.query,
            searx_host,
            searx_port
        ).await.map_err(AgentSearchError::ParallelTreeAgentSearchError),
        AgentSearchStrategy::MultiQueryParallelTree => multi_query_parallel_tree_agent_search(
            &query.query,
            searx_host,
            searx_port
        ).await.map_err(AgentSearchError::MultiQueryParallelTreeAgentSearchError),
    }
}

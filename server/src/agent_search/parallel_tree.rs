use crate::agent_search::VisitAndExtractRelevantInfoError;
use crate::agent_search::{
    parallel_visit_and_extract_relevant_info, AgentSearchInput, AnalysisDocument,
    PreFormattedAgentSearchResult, SearchResult,
};
use crate::llm::{default_completion, LLMError};
use crate::prompts::{build_dependency_tree_system_prompt, Prompt};
use crate::search;
use crate::search::{search, SearchError};
use serde::Deserialize;
use thiserror::Error;
use tokio::task::JoinError;

use super::ParallelAgentSearchError;

#[derive(Error, Debug)]
pub enum ParallelTreeAgentSearchError {
    #[error("Search failed: {0}")]
    SearchError(#[from] SearchError),
    #[error("Visit and extract relevant info failed: {0}")]
    VisitAndExtractRelevantInfoError(#[from] VisitAndExtractRelevantInfoError),
    #[error("Tree construction failed: {0}")]
    TreeConstructionError(#[from] TreeConstructionError),
    #[error("Parallel agent search error: {0}")]
    ParallelAgentSearchError(#[from] ParallelAgentSearchError),
    #[error("Join error: {0}")]
    JoinError(#[from] JoinError),
}

#[derive(Error, Debug)]
pub enum TreeConstructionError {
    #[error("LLM error: {0}")]
    LLMError(#[from] LLMError),
    #[error("Parse error: {0}")]
    ParseError(String),
}

#[derive(Deserialize, Debug)]
struct DependencyTree {
    levels: Vec<Vec<usize>>,
}

async fn construct_dependency_tree(
    query: &str,
    search_results: &[SearchResult],
) -> Result<DependencyTree, TreeConstructionError> {
    let results_display = search_results
        .iter()
        .enumerate()
        .map(|(idx, result)| format!("[{}] {} ({})", idx, result.title, result.url))
        .collect::<Vec<_>>()
        .join("\n");

    let prompt = Prompt::new(
        build_dependency_tree_system_prompt(),
        format!(
            "# Query:\n{}\n\n# Search Results:\n{}",
            query, results_display
        ),
    );

    let completion = match default_completion(&prompt).await {
        Ok(completion) => completion,
        Err(e) => return Err(TreeConstructionError::LLMError(e)),
    };

    serde_json::from_str(&completion).map_err(|e| {
        TreeConstructionError::ParseError(format!("Failed to parse dependency tree: {}", e))
    })
}

async fn process_level(
    query: &str,
    search_results: &[SearchResult],
    level_indices: &[usize],
    current_analysis: &str,
) -> Result<String, ParallelTreeAgentSearchError> {
    let level_results: Vec<SearchResult> = level_indices
        .iter()
        .map(|&idx| search_results[idx].clone())
        .collect();
    let aggregated_result =
        match parallel_visit_and_extract_relevant_info(query, &level_results, current_analysis)
            .await
        {
            Ok(result) => result,
            Err(e) => return Err(ParallelTreeAgentSearchError::ParallelAgentSearchError(e)),
        };
    Ok(aggregated_result.raw_analysis.content)
}

pub async fn parallel_tree_agent_search(
    search_input: &AgentSearchInput,
    searx_host: &str,
    searx_port: &str,
) -> Result<PreFormattedAgentSearchResult, ParallelTreeAgentSearchError> {
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
        Err(e) => return Err(ParallelTreeAgentSearchError::SearchError(e)),
    };

    let dependency_tree = construct_dependency_tree(&search_input.query, &search_results)
        .await
        .map_err(ParallelTreeAgentSearchError::TreeConstructionError)?;

    let mut current_analysis = String::new();
    let mut visited_results = Vec::new();

    for level in dependency_tree.levels {
        current_analysis = process_level(
            &search_input.query,
            &search_results,
            &level,
            &current_analysis,
        )
        .await?;
        visited_results.extend(level.iter().map(|&idx| search_results[idx].clone()));
    }

    Ok(PreFormattedAgentSearchResult {
        raw_analysis: AnalysisDocument {
            content: current_analysis,
            visited_results,
            unvisited_results: Vec::new(),
        },
        queries_executed: vec![search_input.query.clone()],
    })
}

use crate::agent_search::VisitAndExtractRelevantInfoError;
use crate::agent_search::{
    parallel_visit_and_extract_relevant_info, AgentSearchResult, AnalysisDocument, SearchResult,
};
use crate::llm::{CompletionBuilder, LLMError, Model, Provider};
use crate::prompts::{build_dependency_tree_system_prompt, Prompt};
use crate::search::{search, SearchError};
use serde::Deserialize;
use std::fmt::Display;
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
pub struct TreeConstructionError(LLMError);

impl Display for TreeConstructionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Failed to construct tree: {}", self.0)
    }
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

    let completion = CompletionBuilder::new()
        .model(Model::Claude35Sonnet)
        .provider(Provider::Anthropic)
        .messages(prompt.build_messages())
        .temperature(0.0)
        .build()
        .await
        .map_err(TreeConstructionError)?;

    serde_json::from_str(&completion).map_err(|e| {
        TreeConstructionError(LLMError::ParseError(format!(
            "Failed to parse dependency tree: {}",
            e
        )))
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
    Ok(aggregated_result.analysis.content)

    // let extraction_tasks = level_results
    //     .iter()
    //     .map(|result| {
    //         let query = query.to_string();
    //         let current_analysis = current_analysis.to_string();
    //         let result = result.clone();
    //         task::spawn(async move {
    //             visit_and_extract_relevant_info(query.as_str(), current_analysis.as_str(), &result)
    //                 .await
    //         })
    //     })
    //     .collect::<Vec<_>>();

    // let extraction_results = join_all(extraction_tasks)
    //     .await
    //     .into_iter()
    //     .collect::<Result<Vec<_>, _>>()
    //     .into_iter()
    //     .enumerate()
    //     .map(|(batch_index, batch_result)| {
    //         batch_result
    //             .into_iter()
    //             .enumerate()
    //             .map(|(index_in_batch, content)| ExtractionResult {
    //                 search_result: level_results[batch_index * level_results[] + index_in_batch].clone(),
    //                 content,
    //             })
    //             .map_err(ParallelTreeAgentSearchError::VisitAndExtractRelevantInfoError)
    //     })
    //     .collect::<Result<Vec<_>, _>>()?;

    // let aggregated_content = if !extraction_results.is_empty() {
    //     let extraction_results_with_metadata = extraction_results
    //         .into_iter()
    //         .zip(level_results.iter())
    //         .map(|(content, result)| ExtractionResult {
    //             search_result: result.clone(),
    //             content,
    //         })
    //         .collect();

    //     aggregate_results(query, extraction_results_with_metadata)
    //         .await
    //         .map_err(|e| {
    //             ParallelTreeAgentSearchError::TreeConstructionError(TreeConstructionError(e.0))
    //         })?
    // } else {
    //     current_analysis.to_string()
    // };

    // Ok(aggregated_content)
}

pub async fn parallel_tree_agent_search(
    query: &str,
    searx_host: &str,
    searx_port: &str,
) -> Result<AgentSearchResult, ParallelTreeAgentSearchError> {
    let search_results = search(query, searx_host, searx_port)
        .await
        .map_err(ParallelTreeAgentSearchError::SearchError)?;

    let dependency_tree = construct_dependency_tree(query, &search_results)
        .await
        .map_err(ParallelTreeAgentSearchError::TreeConstructionError)?;

    let mut current_analysis = String::new();
    let mut visited_results = Vec::new();

    for level in dependency_tree.levels {
        current_analysis = process_level(query, &search_results, &level, &current_analysis).await?;
        visited_results.extend(level.iter().map(|&idx| search_results[idx].clone()));
    }

    Ok(AgentSearchResult {
        analysis: AnalysisDocument {
            content: current_analysis,
            visited_results,
            unvisited_results: Vec::new(),
        },
        raw_results: search_results,
    })
}

// async fn aggregate_results(
//     query: &str,
//     extraction_results: Vec<ExtractionResult>,
// ) -> Result<String, AggregationPassError> {
//     let extraction_results_display = extraction_results
//         .iter()
//         .map(|result| {
//             format!(
//                 "## {} ({})\n\n{}",
//                 result.search_result.title, result.search_result.url, result.content
//             )
//         })
//         .collect::<Vec<_>>()
//         .join("\n\n");

//     let prompt = Prompt::new(
//         AGGREGATE_WEB_SEARCH_FINDINGS_PROMPT.to_string(),
//         format!(
//             r#"# Search query
// {query}

// # Extracted information
// {extraction_results_display}"#
//         ),
//     );

//     CompletionBuilder::new()
//         .model(Model::Claude35Sonnet)
//         .provider(Provider::Anthropic)
//         .messages(prompt.build_messages())
//         .temperature(0.0)
//         .build()
//         .await
//         .map_err(AggregationPassError)
// }

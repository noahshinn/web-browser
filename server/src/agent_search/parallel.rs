use crate::agent_search::{
    visit_and_extract_relevant_info, AggregationPassError, VisitAndExtractRelevantInfoError,
};
use crate::agent_search::{AgentSearchResult, AnalysisDocument, SearchResult};
use crate::llm::{CompletionBuilder, Model, Provider};
use crate::prompts::{Prompt, AGGREGATE_WEB_SEARCH_FINDINGS_PROMPT};
use crate::search::search;
use crate::search::SearchError;
use futures::future::join_all;
use serde::Deserialize;
use thiserror::Error;
use tokio::task;
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

#[derive(Deserialize, Debug, Clone)]
struct ExtractionResult {
    search_result: SearchResult,
    content: String,
}

pub async fn parallel_agent_search(
    query: &str,
    searx_host: &str,
    searx_port: &str,
) -> Result<AgentSearchResult, ParallelAgentSearchError> {
    let search_results = match search(query, searx_host, searx_port).await {
        Ok(results) => results,
        Err(e) => return Err(ParallelAgentSearchError::SearchError(e)),
    };
    let extraction_tasks = search_results
        .iter()
        .map(|result| {
            let query = query.to_string();
            let result = result.clone();
            task::spawn(async move {
                visit_and_extract_relevant_info(query.as_str(), "", &result).await
            })
        })
        .collect::<Vec<_>>();
    let extraction_results: Vec<ExtractionResult> = join_all(extraction_tasks)
        .await
        .into_iter()
        .collect::<Result<Vec<_>, _>>()?
        .into_iter()
        .enumerate()
        .map(|(index, result)| {
            result
                .map(|content| ExtractionResult {
                    search_result: search_results[index].clone(),
                    content,
                })
                .map_err(ParallelAgentSearchError::VisitAndExtractRelevantInfoError)
        })
        .collect::<Result<Vec<_>, _>>()?;
    let aggregated_result = aggregate_results(query, extraction_results).await?;
    Ok(AgentSearchResult {
        analysis: AnalysisDocument {
            content: aggregated_result,
            visited_results: search_results.clone(),
            unvisited_results: Vec::new(),
        },
        raw_results: search_results,
    })
}

async fn aggregate_results(
    query: &str,
    extraction_results: Vec<ExtractionResult>,
) -> Result<String, AggregationPassError> {
    let extraction_results_display = extraction_results
        .iter()
        .map(|result| {
            format!(
                "## {} ({})\n\n{}",
                result.search_result.title, result.search_result.url, result.content
            )
        })
        .collect::<Vec<_>>()
        .join("\n\n");
    let user_prompt = format!(
        r#"# Search query
{query}

# Extracted information
{extraction_results_display}"#
    );
    let prompt = Prompt::new(
        AGGREGATE_WEB_SEARCH_FINDINGS_PROMPT.to_string(),
        user_prompt,
    );
    let completion = match CompletionBuilder::new()
        .model(Model::Claude35Sonnet)
        .provider(Provider::Anthropic)
        .messages(prompt.build_messages())
        .temperature(0.0)
        .build()
        .await
    {
        Ok(completion) => completion,
        Err(e) => return Err(AggregationPassError(e)),
    };
    Ok(completion)
}

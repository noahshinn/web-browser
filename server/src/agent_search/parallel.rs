use thiserror::Error;
use crate::agent_search::{AgentSearchResult, AnalysisDocument};
use crate::search::SearchError;
use crate::agent_search::{VisitAndExtractRelevantInfoError, AggregationPassError, visit_and_extract_relevant_info};
use crate::search::search;
use crate::prompts::{AGGREGATE_WEB_SEARCH_FINDINGS_PROMPT, Prompt};
use futures::future::join_all;
use tokio::task;

#[derive(Error, Debug)]
pub enum ParallelAgentSearchError {
    #[error("Search failed: {0}")]
    SearchError(#[from] SearchError),
    #[error("Visit and extract relevant info failed: {0}")]
    VisitAndExtractRelevantInfoError(#[from] VisitAndExtractRelevantInfoError),
    #[error("Aggregation pass failed: {0}")]
    AggregationPassError(#[from] AggregationPassError),
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
    let extraction_tasks = search_results.iter().map(|result| {
        let query = query.to_string();
        task::spawn(async move {
            visit_and_extract_relevant_info(
                query.as_str(),
                "",
                result,
            ).await
        })
    }).collect::<Vec<_>>();
    let extraction_results = join_all(extraction_tasks).await
        .into_iter()
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| ParallelAgentSearchError::VisitAndExtractRelevantInfoError(e))?
        .into_iter()
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| ParallelAgentSearchError::VisitAndExtractRelevantInfoError(e))?;
    let aggregated_result = aggregate_results(query, extraction_results).await?;
    Ok(AgentSearchResult {
        analysis: AnalysisDocument {
            content: aggregated_result,
            visited_results: search_results,
            unvisited_results: Vec::new(),
        },
        raw_results: search_results,
    })
}

async fn aggregate_results(
    query: &str,
    extraction_results: Vec<ExtractionResult>,
) -> Result<String, AggregationPassError> {
    let extraction_results_display = extraction_results.iter().map(|result| {
        format!(
            "## {} ({})\n\n{}",
            result.search_result.title,
            result.search_result.url,
            result.content
        )
    }).collect::<Vec<_>>().join("\n\n");
    let user_prompt = format!(
        r#"# Search query
{query}

# Extracted information
{extraction_results_display}"#
    );
    let prompt = Prompt::new(AGGREGATE_WEB_SEARCH_FINDINGS_PROMPT.to_string(), user_prompt);
    match prompt.send_message(query, extraction_results).await {
        Ok(response) => Ok(response),
        Err(e) => Err(AggregationPassError::LLMError(e)),
    }
}

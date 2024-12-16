use crate::agent_search::{
    check_sufficient_information, visit_and_extract_relevant_info, AgentSearchResult,
    AnalysisDocument, SearchQuery, SufficientInformationCheckError,
    VisitAndExtractRelevantInfoError,
};
use crate::search;
use crate::search::{search, SearchError};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum SequentialAgentSearchError {
    #[error("Search failed: {0}")]
    SearchError(#[from] SearchError),
    #[error("Visit and extract relevant info failed: {0}")]
    VisitAndExtractRelevantInfoError(#[from] VisitAndExtractRelevantInfoError),
    #[error("Sufficient information check failed: {0}")]
    SufficientInformationCheckError(#[from] SufficientInformationCheckError),
}

pub async fn sequential_agent_search(
    query: &SearchQuery,
    searx_host: &str,
    searx_port: &str,
) -> Result<AgentSearchResult, SequentialAgentSearchError> {
    let search_result = match search(
        &search::SearchQuery {
            query: query.query.clone(),
            max_results_to_visit: query.max_results_to_visit,
        },
        searx_host,
        searx_port,
    )
    .await
    {
        Ok(results) => results,
        Err(e) => return Err(SequentialAgentSearchError::SearchError(e)),
    };
    let mut analysis = AnalysisDocument {
        content: String::new(),
        visited_results: Vec::new(),
        unvisited_results: search_result.clone(),
    };
    while !analysis.unvisited_results.is_empty() {
        let result = analysis.unvisited_results.remove(0);
        let new_analysis =
            match visit_and_extract_relevant_info(&query.query, &analysis.content, &result).await {
                Ok(new_analysis) => new_analysis,
                Err(e) => {
                    return Err(SequentialAgentSearchError::VisitAndExtractRelevantInfoError(e))
                }
            };
        analysis.content = new_analysis;
        analysis.visited_results.push(result);
        match check_sufficient_information(
            &query.query,
            &analysis.content,
            &analysis.visited_results,
            &analysis.unvisited_results,
        )
        .await
        {
            Ok(decision) => {
                if decision.sufficient {
                    break;
                }
            }
            Err(e) => {
                return Err(SequentialAgentSearchError::SufficientInformationCheckError(
                    e,
                ))
            }
        }
    }
    Ok(AgentSearchResult {
        analysis,
        raw_results: search_result,
    })
}

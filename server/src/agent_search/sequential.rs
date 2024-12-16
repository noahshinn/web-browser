use crate::agent_search::{
    check_sufficient_information, visit_and_extract_relevant_info, SufficientInformationCheckError,
    VisitAndExtractRelevantInfoError,
};
use crate::agent_search::{AgentSearchResult, AnalysisDocument};
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
    query: &str,
    searx_host: &str,
    searx_port: &str,
) -> Result<AgentSearchResult, SequentialAgentSearchError> {
    let search_result = match search(query, searx_host, searx_port).await {
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
            visit_and_extract_relevant_info(query, &analysis.content, &result).await?;
        analysis.content = new_analysis;
        analysis.visited_results.push(result);
        match check_sufficient_information(
            query,
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

use crate::llm::{CompletionBuilder, LLMError, Model, Provider};
use crate::prompts::{
    Prompt, RESULT_FORMAT_ANSWER_SYSTEM_PROMPT, RESULT_FORMAT_CUSTOM_SYSTEM_PROMPT,
    RESULT_FORMAT_FAQ_SYSTEM_PROMPT, RESULT_FORMAT_NEWS_ARTICLE_SYSTEM_PROMPT,
    RESULT_FORMAT_RESEARCH_SUMMARY_SYSTEM_PROMPT, RESULT_FORMAT_WEBPAGE_SYSTEM_PROMPT,
};
use crate::search::SearchResult;
use rocket::form::FromFormField;
use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum ResultFormatError {
    #[error("Failed to format result: {0}")]
    LLMError(#[from] LLMError),
    #[error("Custom format description is missing")]
    CustomFormatDescriptionMissing,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct AnalysisDocument {
    pub content: String,
    pub visited_results: Vec<SearchResult>,
    pub unvisited_results: Vec<SearchResult>,
}

#[derive(Serialize, Deserialize, Debug, Clone, FromFormField)]
pub enum ResultFormat {
    #[serde(rename = "answer")]
    Answer,
    #[serde(rename = "research_summary")]
    ResearchSummary,
    #[serde(rename = "faq_article")]
    FAQArticle,
    #[serde(rename = "news_article")]
    NewsArticle,
    #[serde(rename = "webpage")]
    Webpage,
    #[serde(rename = "custom")]
    Custom,
}

impl Default for ResultFormat {
    fn default() -> Self {
        ResultFormat::Answer
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum ResultFormatResponse {
    #[serde(rename = "answer")]
    Answer(String),
    #[serde(rename = "research_summary")]
    ResearchSummary(String),
    #[serde(rename = "faq_article")]
    FAQArticle(Article),
    #[serde(rename = "news_article")]
    NewsArticle(Article),
    #[serde(rename = "webpage")]
    Webpage(Article),
    #[serde(rename = "custom")]
    Custom(String),
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Article {
    pub title: String,
    pub content: String,
}

pub async fn format_result(
    query: &str,
    analysis_document: &AnalysisDocument,
    result_format: &ResultFormat,
    custom_format_description: Option<&str>,
) -> Result<ResultFormatResponse, ResultFormatError> {
    match result_format {
        ResultFormat::Answer => format_result_answer(query, analysis_document).await,
        ResultFormat::ResearchSummary => {
            format_result_research_summary(query, analysis_document).await
        }
        ResultFormat::FAQArticle => format_result_faq(query, analysis_document).await,
        ResultFormat::NewsArticle => format_result_news_article(analysis_document).await,
        ResultFormat::Webpage => format_result_webpage(analysis_document).await,
        ResultFormat::Custom => {
            if let Some(custom_format_description) = custom_format_description {
                format_result_custom(query, analysis_document, custom_format_description).await
            } else {
                Err(ResultFormatError::CustomFormatDescriptionMissing)
            }
        }
    }
}

pub async fn format_result_answer(
    query: &str,
    analysis_document: &AnalysisDocument,
) -> Result<ResultFormatResponse, ResultFormatError> {
    let prompt = Prompt {
        instruction: RESULT_FORMAT_ANSWER_SYSTEM_PROMPT.to_string(),
        context: format!(
            "# Query:\n{}\n\n# Search results:\n{}",
            query,
            analysis_document
                .visited_results
                .iter()
                .map(|r| format!("## {} ({})\n\n{}", r.title, r.url, r.content))
                .collect::<Vec<String>>()
                .join("\n\n")
        ),
    };
    let completion = match CompletionBuilder::new()
        .model(Model::Claude35Sonnet)
        .provider(Provider::Anthropic)
        .messages(prompt.build_messages())
        .temperature(0.0)
        .build()
        .await
    {
        Ok(completion) => completion,
        Err(e) => return Err(ResultFormatError::LLMError(e)),
    };
    Ok(ResultFormatResponse::Answer(completion))
}

pub async fn format_result_research_summary(
    query: &str,
    analysis_document: &AnalysisDocument,
) -> Result<ResultFormatResponse, ResultFormatError> {
    let prompt = Prompt {
        instruction: RESULT_FORMAT_RESEARCH_SUMMARY_SYSTEM_PROMPT.to_string(),
        context: format!(
            "# Query:\n{}\n\n# Search results:\n{}",
            query,
            analysis_document
                .visited_results
                .iter()
                .map(|r| format!("## {} ({})\n\n{}", r.title, r.url, r.content))
                .collect::<Vec<String>>()
                .join("\n\n")
        ),
    };
    let completion = match CompletionBuilder::new()
        .model(Model::Claude35Sonnet)
        .provider(Provider::Anthropic)
        .messages(prompt.build_messages())
        .temperature(0.0)
        .build()
        .await
    {
        Ok(completion) => completion,
        Err(e) => return Err(ResultFormatError::LLMError(e)),
    };
    Ok(ResultFormatResponse::ResearchSummary(completion))
}

pub async fn format_result_faq(
    query: &str,
    analysis_document: &AnalysisDocument,
) -> Result<ResultFormatResponse, ResultFormatError> {
    let prompt = Prompt {
        instruction: RESULT_FORMAT_FAQ_SYSTEM_PROMPT.to_string(),
        context: format!(
            "# Query:\n{}\n\n# Search results:\n{}",
            query,
            analysis_document
                .visited_results
                .iter()
                .map(|r| format!("## {} ({})\n\n{}", r.title, r.url, r.content))
                .collect::<Vec<String>>()
                .join("\n\n")
        ),
    };
    let completion = match CompletionBuilder::new()
        .model(Model::Claude35Sonnet)
        .provider(Provider::Anthropic)
        .messages(prompt.build_messages())
        .temperature(0.0)
        .build()
        .await
    {
        Ok(completion) => completion,
        Err(e) => return Err(ResultFormatError::LLMError(e)),
    };
    Ok(ResultFormatResponse::FAQArticle(Article {
        title: "Frequently Asked Questions".to_string(),
        content: completion,
    }))
}

pub async fn format_result_news_article(
    analysis_document: &AnalysisDocument,
) -> Result<ResultFormatResponse, ResultFormatError> {
    let prompt = Prompt {
        instruction: RESULT_FORMAT_NEWS_ARTICLE_SYSTEM_PROMPT.to_string(),
        context: format!(
            "# Search results:\n{}",
            analysis_document
                .visited_results
                .iter()
                .map(|r| format!("## {} ({})\n\n{}", r.title, r.url, r.content))
                .collect::<Vec<String>>()
                .join("\n\n")
        ),
    };
    let completion = match CompletionBuilder::new()
        .model(Model::Claude35Sonnet)
        .provider(Provider::Anthropic)
        .messages(prompt.build_messages())
        .temperature(0.0)
        .build()
        .await
    {
        Ok(completion) => completion,
        Err(e) => return Err(ResultFormatError::LLMError(e)),
    };
    let parts: Vec<&str> = completion.splitn(2, "\n").collect();
    let (title, content) = match parts.as_slice() {
        [title, content] => (title.to_string(), content.to_string()),
        _ => ("News Article".to_string(), completion),
    };
    Ok(ResultFormatResponse::NewsArticle(Article {
        title,
        content,
    }))
}

pub async fn format_result_webpage(
    analysis_document: &AnalysisDocument,
) -> Result<ResultFormatResponse, ResultFormatError> {
    let prompt = Prompt {
        instruction: RESULT_FORMAT_WEBPAGE_SYSTEM_PROMPT.to_string(),
        context: format!(
            "# Search results:\n{}",
            analysis_document
                .visited_results
                .iter()
                .map(|r| format!("## {} ({})\n\n{}", r.title, r.url, r.content))
                .collect::<Vec<String>>()
                .join("\n\n")
        ),
    };
    let completion = match CompletionBuilder::new()
        .model(Model::Claude35Sonnet)
        .provider(Provider::Anthropic)
        .messages(prompt.build_messages())
        .temperature(0.0)
        .build()
        .await
    {
        Ok(completion) => completion,
        Err(e) => return Err(ResultFormatError::LLMError(e)),
    };
    let parts: Vec<&str> = completion.splitn(2, "\n").collect();
    let (title, content) = match parts.as_slice() {
        [title, content] => (title.to_string(), content.to_string()),
        _ => ("Webpage".to_string(), completion),
    };
    Ok(ResultFormatResponse::Webpage(Article { title, content }))
}

pub async fn format_result_custom(
    query: &str,
    analysis_document: &AnalysisDocument,
    custom_format_description: &str,
) -> Result<ResultFormatResponse, ResultFormatError> {
    let prompt = Prompt {
        instruction: RESULT_FORMAT_CUSTOM_SYSTEM_PROMPT.to_string(),
        context: format!(
            "# Custom format description:\n{}\n\n# Query:\n{}\n\n# Search results:\n{}",
            custom_format_description,
            query,
            analysis_document
                .visited_results
                .iter()
                .map(|r| format!("## {} ({})\n\n{}", r.title, r.url, r.content))
                .collect::<Vec<String>>()
                .join("\n\n")
        ),
    };
    let completion = match CompletionBuilder::new()
        .model(Model::Claude35Sonnet)
        .provider(Provider::Anthropic)
        .messages(prompt.build_messages())
        .temperature(0.0)
        .build()
        .await
    {
        Ok(completion) => completion,
        Err(e) => return Err(ResultFormatError::LLMError(e)),
    };
    Ok(ResultFormatResponse::Custom(completion))
}

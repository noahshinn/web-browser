use crate::llm::{CompletionOptions, Message, Model, LLMError};
use reqwest::header::{HeaderMap, HeaderValue, AUTHORIZATION, CONTENT_TYPE};
use serde::{Deserialize, Serialize};
use std::env;

const OPENAI_API_URL: &str = "https://api.openai.com/v1/chat/completions";

#[derive(Serialize)]
struct OpenAIRequest<'a> {
    model: String,
    messages: &'a [Message],
    #[serde(skip_serializing_if = "Option::is_none")]
    temperature: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    max_tokens: Option<i32>,
}

#[derive(Deserialize)]
struct OpenAIResponse {
    choices: Vec<OpenAIChoice>,
}

#[derive(Deserialize)]
struct OpenAIChoice {
    message: OpenAIMessage,
}

#[derive(Deserialize)]
struct OpenAIMessage {
    content: String,
}

pub(crate) async fn completion_openai(
    model: Model,
    messages: &[Message],
    options: Option<&CompletionOptions>,
) -> Result<String, LLMError> {
    let req_body = OpenAIRequest {
        model: model.to_string(),
        messages,
        temperature: options.and_then(|opt| (opt.temperature != 0.0).then_some(opt.temperature)),
        max_tokens: options.and_then(|opt| {
            (opt.max_completion_tokens != 0).then_some(opt.max_completion_tokens)
        }),
    };

    let api_key = match env::var("OPENAI_API_KEY") {
        Ok(key) => key,
        Err(_) => return Err(LLMError::RequestBuildingError(
            "OPENAI_API_KEY environment variable not set".to_string()
        )),
    };

    let mut headers = HeaderMap::new();
    
    let auth_header = match HeaderValue::from_str(&format!("Bearer {api_key}")) {
        Ok(header) => header,
        Err(e) => return Err(LLMError::RequestBuildingError(e.to_string())),
    };
    headers.insert(AUTHORIZATION, auth_header);
    headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));

    let client = reqwest::Client::new();
    let response = match client
        .post(OPENAI_API_URL)
        .headers(headers)
        .json(&req_body)
        .send()
        .await
    {
        Ok(resp) => resp,
        Err(e) => return Err(LLMError::RequestError(e)),
    };

    let response_body: OpenAIResponse = match response.json().await {
        Ok(body) => body,
        Err(e) => return Err(LLMError::RequestError(e)),
    };

    response_body
        .choices
        .first()
        .map(|choice| choice.message.content.clone())
        .ok_or(LLMError::EmptyResponse)
}

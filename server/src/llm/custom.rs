use crate::llm::{CompletionOptions, Message, Model, LLMError};
use reqwest::header::{HeaderMap, HeaderValue, CONTENT_TYPE};
use serde::{Deserialize, Serialize};

#[derive(Serialize)]
struct CustomRequest<'a> {
    model: String,
    messages: &'a [Message],
    stream: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    temperature: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    max_tokens: Option<i32>,
}

#[derive(Deserialize)]
struct CustomResponse {
    message: CustomMessage,
}

#[derive(Deserialize)]
struct CustomMessage {
    content: String,
}

pub async fn completion_custom(
    _model: Model,
    messages: &[Message],
    options: Option<&CompletionOptions>,
) -> Result<String, LLMError> {
    let Some(options) = options else {
        return Err(LLMError::RequestBuildingError("completion options not set".to_string()));
    };

    let custom_endpoint = match &options.custom_server_endpoint {
        Some(endpoint) => endpoint,
        None => return Err(LLMError::RequestBuildingError("custom server endpoint not set".to_string())),
    };

    let custom_model = match &options.custom_model {
        Some(model) => model,
        None => return Err(LLMError::RequestBuildingError("custom model not set".to_string())),
    };

    let req_body = CustomRequest {
        model: custom_model.clone(),
        messages: messages,
        stream: false,
        temperature: (options.temperature != 0.0).then_some(options.temperature),
        max_tokens: (options.max_completion_tokens != 0).then_some(options.max_completion_tokens),
    };

    let mut headers = HeaderMap::new();
    headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));

    let client = reqwest::Client::new();
    let response = match client
        .post(custom_endpoint)
        .headers(headers)
        .json(&req_body)
        .send()
        .await
    {
        Ok(resp) => resp,
        Err(e) => return Err(LLMError::RequestError(e)),
    };

    let response_body: CustomResponse = match response.json().await {
        Ok(body) => body,
        Err(e) => return Err(LLMError::RequestError(e)),
    };

    Ok(response_body.message.content)
} 
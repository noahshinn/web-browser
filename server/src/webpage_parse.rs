use thiserror::Error;

use crate::utils::enforce_n_sequential_newlines;

use ammonia::Builder;
use reqwest;
use std::collections::HashSet;

#[derive(Error, Debug)]
pub enum WebpageParseError {
    #[error("Failed to fetch webpage: {0}")]
    FetchError(#[from] reqwest::Error),
    #[error("Failed to parse webpage")]
    DomParseError(#[from] DomParseError),
    #[error("Failed to clean webpage: {0}")]
    SemanticParseError(#[from] SemanticParseError),
}

#[derive(Error, Debug)]
pub enum DomParseError {
    #[error("Failed to parse webpage")]
    ParseError(String),
}

#[derive(Error, Debug)]
pub enum SemanticParseError {
    #[error("Failed to parse webpage content: {0}")]
    ParseError(String),
}

#[derive(Clone)]
pub struct ParsedWebpage {
    pub original_content: String,
    pub content: String,
}

const MAX_RETRIES: u32 = 3;

pub async fn visit_and_parse_webpage(url: &str) -> Result<ParsedWebpage, WebpageParseError> {
    let mut attempts = 0;
    let response = loop {
        let client = reqwest::Client::builder()
            .gzip(true)
            .build()
            .map_err(WebpageParseError::FetchError)?;
        match client.get(url)
            .header("accept", "text/html,application/xhtml+xml,application/xml;q=0.9,image/avif,image/webp,image/apng,*/*;q=0.8,application/signed-exchange;v=b3;q=0.7")
            .header("Accept-Language", "en-US,en;q=0.9")
            .header("priority", "u=0, i")
            .header("sec-ch-ua", "\"Chromium\";v=\"128\", \"Not;A=Brand\";v=\"24\", \"Google Chrome\";v=\"128\"")
            .header("sec-ch-ua-mobile", "?0")
            .header("sec-ch-ua-platform", "\"macOS\"")
            .header("sec-fetch-dest", "document")
            .header("sec-fetch-mode", "navigate")
            .header("sec-fetch-site", "none")
            .header("sec-fetch-user", "?1")
            .header("upgrade-insecure-requests", "1")
            .header("user-agent", "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/128.0.0.0 Safari/537.36")
            .header("Accept-Encoding", "gzip")
            .send()
            .await
        {
            Ok(response) => break response,
            Err(e) => {
                attempts += 1;
                if attempts >= MAX_RETRIES {
                    return Err(WebpageParseError::FetchError(e));
                }
                tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
            }
        }
    };
    let webpage_text = match response.text().await {
        Ok(text) => text,
        Err(e) => return Err(WebpageParseError::FetchError(e)),
    };
    let dom_text = match dom_parse_webpage(&webpage_text) {
        Ok(text) => text,
        Err(e) => return Err(WebpageParseError::DomParseError(e)),
    };
    let trimmed_text = dom_text.content.trim();
    Ok(ParsedWebpage {
        original_content: dom_text.original_content,
        content: trimmed_text.to_string(),
    })
}

const WHITELISTED_ATTRIBUTES: [&str; 10] = [
    "data-label",
    "href",
    "label",
    "alt",
    "title",
    "aria-label",
    "aria-description",
    "role",
    "type",
    "name",
];
const BLACKLISTED_TAGS: [&str; 27] = [
    "abbr",
    "script",
    "style",
    "noscript",
    "iframe",
    "svg",
    "span",
    "cite",
    "i",
    "b",
    "u",
    "em",
    "strong",
    "small",
    "s",
    "q",
    "figcaption",
    "figure",
    "footer",
    "header",
    "nav",
    "section",
    "article",
    "aside",
    "main",
    "canvas",
    "center",
];

fn dom_parse_webpage(webpage_text: &str) -> Result<ParsedWebpage, DomParseError> {
    let clean_html = Builder::new()
        .rm_tags(BLACKLISTED_TAGS)
        .generic_attributes(HashSet::from_iter(WHITELISTED_ATTRIBUTES))
        .attribute_filter(|element, attribute, value| match (element, attribute) {
            ("div", "src") => None,
            ("img", "src") => None,
            ("img", "height") => None,
            ("img", "width") => None,
            ("a", "rel") => None,
            _ => Some(value.into()),
        })
        .strip_comments(true)
        .clean(&webpage_text)
        .to_string();
    let clean_html = clean_html
        .lines()
        .filter(|line| !line.trim().is_empty())
        .collect::<Vec<&str>>()
        .join("\n");
    let clean_html = enforce_n_sequential_newlines(&clean_html, 2);
    Ok(ParsedWebpage {
        original_content: webpage_text.to_string(),
        content: clean_html,
    })
}

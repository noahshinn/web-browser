use reqwest;
use thiserror::Error;
use ammonia::Builder;
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

pub struct ParsedWebpage {
    pub original_content: String,
    pub content: String,
}

pub async fn visit_and_parse_webpage(url: &str) -> Result<ParsedWebpage, WebpageParseError> {
    let response = match reqwest::get(url).await {
        Ok(response) => response,
        Err(e) => return Err(WebpageParseError::FetchError(e)),
    };
    let webpage_text = response.text().await.map_err(|e| {
        WebpageParseError::FetchError(e)
    })?;

    let dom_text = dom_parse_webpage(&webpage_text)?;
    // let semantic_text = semantic_parse_webpage(&dom_text).await?;
    // trim the leading and trailing whitespace
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
        .attribute_filter(|element, attribute, value| {
            match (element, attribute) {
                ("div", "src") => None,
                ("img", "src") => None,
                ("img", "height") => None,
                ("img", "width") => None,
                ("a", "rel") => None,
                _ => Some(value.into())
            }
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

fn enforce_n_sequential_newlines(text: &str, n: usize) -> String {
    let mut result = String::with_capacity(text.len());
    let mut newline_count = 0;
    for c in text.chars() {
        if c == '\n' {
            newline_count += 1;
            if newline_count <= n {
                result.push(c);
            }
        } else {
            newline_count = 0;
            result.push(c);
        }
    }
    result
}

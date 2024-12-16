use crate::llm::{Message, Role};

pub struct Prompt {
    pub instruction: String,
    pub context: String,
}

impl Prompt {
    pub fn new(instruction: String, context: String) -> Self {
        Self { instruction, context }
    }

    pub fn build_messages(self) -> Vec<Message> {
        vec![
            Message {
                role: Role::System,
                content: self.instruction,
            },
            Message {
                role: Role::User,
                content: self.context,
            },
        ]
    }
}

pub const WEB_SEARCH_CONTEXT: &str = r#"You are serving a verify specific task within a web search tool for a large language model.
The context is that you are looping over a set of web search results to build a "findings" document."#;

pub const WEB_SEARCH_USE_SAME_WEB_SEARCH_FINDINGS_DOCUMENT: &str = "USE_SAME_WEB_SEARCH_FINDINGS_DOCUMENT";

pub fn build_analyze_result_system_prompt() -> String {
    format!(
        r#"# Task
You will be given a search query and an in-progress analysis document that contains web search findings and a new search result to analyze.
The current web search result may not be the last result, so you should not assume that it is the last result and that you have to add irrelevant information to the analysis for the sake of being comprehensive.
Your task is to read the contents of the current web search result and extract the relevant information (if any) according to the query.
Examples of relevant information include:
- Content that is relevant to the query
- Links to pages that are citations to the information that you are including
- Links to pages that should be visited

## General context
{WEB_SEARCH_CONTEXT}

## Format
Respond with the new findings document so far (or `{WEB_SEARCH_USE_SAME_WEB_SEARCH_FINDINGS_DOCUMENT}` - this will be parsed). If you need to add more information to the findings document, do so.
If there is no new information to add, respond with `{WEB_SEARCH_USE_SAME_WEB_SEARCH_FINDINGS_DOCUMENT}`.
If you respond with `{WEB_SEARCH_USE_SAME_WEB_SEARCH_FINDINGS_DOCUMENT}`, you will move on to the next web search result to analyze and will keep the same findings document so far.
If there is new information to add, rewrite the findings document so far with the new information.
Make sure to not remove any information that is necessary.
It is okay to reformat the findings document to better fit the new information (the current findings document was written without the context of the new search result that you are analyzing).
This response will be used as the new findings document so far.
Remember, you will be reading the contents of many different web search results, so you do not have to force-fit the findings document to the query.
Most of the time, you will find that there is no new information to add to the findings document so far."#
    )
}

pub fn build_select_next_result_system_prompt() -> String {
    format!(
        r#"# Task
You will be given a search query, an in-progress findings document, a list of visited search results, and a list of unvisited search results.
Your task is to select the next unvisited search result to visit.

## General context
{WEB_SEARCH_CONTEXT}

## Format
Each unvisited search result will be labeled with an index.
You must respond with a JSON object in a markdown code block in the following format:
```json
{{
    "index": <the index of the next unvisited search result to analyze>
}}
```

For example:
```json
{{
    "index": 1 // assuming that the next unvisited search result that you want to visit is at index 1
}}
```
"#
    )
}

pub fn build_sufficient_information_check_prompt() -> String {
    format!(
        r#"# Task
You will be given a search query, an in-progress findings document, a list of visited search results, and a list of unvisited search results.
Your task is to determine if the findings document is sufficient to answer the query in full.
If the findings document is not sufficient or if there is an unvisited search result that should be visited before making a decision, return false.
Otherwise, return true.

## General context
{WEB_SEARCH_CONTEXT}

## Format
Respond with a JSON object in a markdown code block in the following format:
```json
{{
    "sufficient": <true or false>
}}
```"#
    )
}

pub const AGGREGATE_WEB_SEARCH_FINDINGS_PROMPT: &str = r#"# Task
You will be given a search query and a list of extracted information from visited search results.
Your task is to aggregate the information from the visited search results into a single document.

## Format
Your response will be directly used as the document. Write it in markdown."#;

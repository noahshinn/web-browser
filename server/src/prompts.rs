use crate::llm::{Message, Role};

pub struct Prompt {
    pub instruction: String,
    pub context: String,
}

impl Prompt {
    pub fn new(instruction: String, context: String) -> Self {
        Self {
            instruction,
            context,
        }
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

pub const WEB_SEARCH_USE_SAME_WEB_SEARCH_FINDINGS_DOCUMENT: &str =
    "USE_SAME_WEB_SEARCH_FINDINGS_DOCUMENT";

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
```
"#
    )
}

pub const AGGREGATE_WEB_SEARCH_FINDINGS_PROMPT: &str = r#"# Task
You will be given a search query and a list of extracted information from visited search results.
Your task is to aggregate the information from the visited search results into a single document.

## Format
Your response will be directly used as the document. Write it in markdown."#;

pub fn build_dependency_tree_system_prompt() -> String {
    format!(
        r#"# Task
You will be given a search query and a list of search results.
Your task is to organize these results into levels based on their dependencies.
Results that depend on information from other results should be placed in later levels.
Results that can be processed independently should be in the same level.
The goal is to create a dependency tree that outlines the fastest way to process the results while not compromising on quality.
Most of the time, you will find that the results are independent and can be processed in parallel.
However, if certain sources should only be visited after others due to a dependency in the information, you should place them in the same level.

## General context
{WEB_SEARCH_CONTEXT}

## Format
Respond with a JSON object in a markdown code block:

```json
{{
    "levels": [
        [0, 2], // Level 0: Results that can be processed first
        [1, 4], // Level 1: Results that depend on level 0
        [3] // Level 2: Results that depend on level 1
    ]
}}
```
"

Each number represents the index of a search result in the input list."#
    )
}

pub const GENERATE_SINGLE_QUERY_SYSTEM_PROMPT: &str = r#"# Task
You will be given a natural language request from a user. Your task is to generate a Google search query that will help find the most relevant information to answer the question.
First, write a reasoning trace, then write the search query. Brainstorm the best place to find the information you need. Your query should search for specific sites, documents, or other information.

## Format
Respond with a JSON object in a markdown code block in the following format:

```json
{
    "reasoning": "the reasoning trace for brainstorming the best query based on the likely location of the information",
    "query": "the search query"
}
```
"#;

pub const GENERATE_PARALLEL_QUERIES_SYSTEM_PROMPT: &str = r#"# Task
You will be given a natural language request from a user. Your task is to generate a list of one or more Google search queries that are required to find the most relevant information to answer the question.
These queries will be searched in parallel and the results will be aggregated at the end.
Most of the time, only one query will be needed.
First, write a reasoning trace, then write the search queries. Brainstorm the best places to find the information you need. Your queries should search for specific sites, documents, or other pieces of information.

## Format
Respond with a JSON object in a markdown code block in the following format:

```json
{
    "reasoning": "the reasoning trace for brainstorming the best queries based on the likely locations of the information",
    "queries": ["query1", "query2", ...]
}
```
"#;

pub const GENERATE_SEQUENTIAL_QUERIES_SYSTEM_PROMPT: &str = r#"# Task
You will be given a natural language request from a user. Your task is to generate a list of one or more Google search queries that are required to find the most relevant information to answer the question.
These queries will be searched in sequence, so write them accordingly.
For example, if information from the first query is necessary to answer the second query, write the first query first and then the second query.
Most of the time, only one query will be needed.
First, write a reasoning trace, then write the search queries. Brainstorm the best places to find the information you need. Your queries should search for specific sites, documents, or other pieces of information.

## Format
Respond with a JSON object in a markdown code block in the following format:

```json
{
    "reasoning": "the reasoning trace for brainstorming the best queries based on the likely locations of the information",
    "queries": ["query1", "query2", ...]
}
```
"#;

pub const RESULT_FORMAT_ANSWER_SYSTEM_PROMPT: &str = r#"# Task
You will be given a search query and a list of search results.
Your task is to answer the query based on the search results.

## Format
Your response will be directly used as the answer. Make it concise and to the point."#;

pub const RESULT_FORMAT_RESEARCH_SUMMARY_SYSTEM_PROMPT: &str = r#"# Task
You will be given a search query and a list of search results.
Your task is to write a research summary of the search results.

## Format
Your response will be directly used as the research summary. Write it in markdown."#;

pub const RESULT_FORMAT_FAQ_SYSTEM_PROMPT: &str = r#"# Task
You will be given a search query and a list of search results.
Your task is to write a FAQ article based on the search results.

## Format
Your response will be directly used as the FAQ article. Write it in markdown in the following form:

# <question>
<answer>
"#;

pub const RESULT_FORMAT_NEWS_ARTICLE_SYSTEM_PROMPT: &str = r#"# Task
You will be given a search query and a list of search results.
Your task is to write a news article based on the search results.

## Format
Your response will be directly used as the news article. Write it in markdown in the following form:

# <title>
<body>
"#;

pub const RESULT_FORMAT_WEBPAGE_SYSTEM_PROMPT: &str = r#"# Task
You will be given a search query and a list of search results.
Your task is to write a webpage based on the search results.

## Format
Your response will be directly used as the webpage. Write it in html in the following form:

<html>
[content]
<html>
"#;

pub const RESULT_FORMAT_CUSTOM_SYSTEM_PROMPT: &str = r#"# Task
You will be given a search query and a list of search results.
Your task is to write a response according to the custom format description.
"#;

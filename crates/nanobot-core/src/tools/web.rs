//! Web search and fetch tools

use async_trait::async_trait;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::time::Duration;
use tracing::info;

use crate::tools::{Tool, ToolError, ToolResult};

// ============================================================================
// Web Search Tool
// ============================================================================

/// Web search provider types
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SearchProvider {
    Brave,
    Tavily,
    DuckDuckGo,
    Jina,
    SearXNG,
}

impl SearchProvider {
    pub fn from_str(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "brave" => Self::Brave,
            "tavily" => Self::Tavily,
            "duckduckgo" | "ddg" => Self::DuckDuckGo,
            "jina" => Self::Jina,
            "searxng" => Self::SearXNG,
            _ => Self::DuckDuckGo, // Default to DDG
        }
    }
}

/// Web search configuration
#[derive(Debug, Clone)]
pub struct WebSearchConfig {
    pub provider: SearchProvider,
    pub api_key: String,
    pub base_url: String,
    pub max_results: u32,
    pub proxy: Option<String>,
}

impl Default for WebSearchConfig {
    fn default() -> Self {
        Self {
            provider: SearchProvider::Brave,
            api_key: String::new(),
            base_url: String::new(),
            max_results: 5,
            proxy: None,
        }
    }
}

/// Search result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchResult {
    pub title: String,
    pub url: String,
    pub snippet: String,
}

/// Web search tool
pub struct WebSearchTool {
    config: WebSearchConfig,
    client: Client,
}

impl WebSearchTool {
    pub fn new(config: WebSearchConfig) -> Self {
        let client = Client::builder()
            .timeout(Duration::from_secs(30))
            .build()
            .expect("Failed to create HTTP client");

        Self { config, client }
    }

    /// Search using Brave Search API
    async fn search_brave(&self, query: &str) -> ToolResult<Vec<SearchResult>> {
        if self.config.api_key.is_empty() {
            return Err(ToolError::Execution(
                "Brave API key not configured".to_string(),
            ));
        }

        let url = format!(
            "https://api.search.brave.com/res/v1/web/search?q={}&count={}",
            urlencoding::encode(query),
            self.config.max_results.min(20)
        );

        let response = self
            .client
            .get(&url)
            .header("Accept", "application/json")
            .header("Accept-Encoding", "gzip")
            .header("X-Subscription-Token", &self.config.api_key)
            .send()
            .await
            .map_err(|e| ToolError::Execution(format!("Search request failed: {}", e)))?;

        if !response.status().is_success() {
            return Err(ToolError::Execution(format!(
                "Brave API error: {}",
                response.status()
            )));
        }

        let data: Value = response.json().await.map_err(|e| {
            ToolError::Execution(format!("Failed to parse response: {}", e))
        })?;

        let results = data
            .get("web")
            .and_then(|w| w.get("results"))
            .and_then(|r| r.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|item| {
                        Some(SearchResult {
                            title: item.get("title")?.as_str()?.to_string(),
                            url: item.get("url")?.as_str()?.to_string(),
                            snippet: item.get("description")?.as_str()?.to_string(),
                        })
                    })
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();

        Ok(results)
    }

    /// Search using Jina Search API
    async fn search_jina(&self, query: &str) -> ToolResult<Vec<SearchResult>> {
        if self.config.api_key.is_empty() {
            return Err(ToolError::Execution(
                "Jina API key not configured".to_string(),
            ));
        }

        let url = "https://s.jina.ai/";

        let response = self
            .client
            .post(url)
            .header("Accept", "application/json")
            .header("Authorization", format!("Bearer {}", self.config.api_key))
            .header("Content-Type", "application/json")
            .body(format!(r#"{{"q": "{}"}}"#, query))
            .send()
            .await
            .map_err(|e| ToolError::Execution(format!("Search request failed: {}", e)))?;

        if !response.status().is_success() {
            return Err(ToolError::Execution(format!(
                "Jina API error: {}",
                response.status()
            )));
        }

        let data: Value = response.json().await.map_err(|e| {
            ToolError::Execution(format!("Failed to parse response: {}", e))
        })?;

        let results = data
            .get("data")
            .and_then(|d| d.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|item| {
                        Some(SearchResult {
                            title: item.get("title")?.as_str()?.to_string(),
                            url: item.get("url")?.as_str()?.to_string(),
                            snippet: item.get("content")?.as_str()?.to_string(),
                        })
                    })
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();

        Ok(results)
    }

    /// Search using DuckDuckGo (via HTML scraping - no API key needed)
    async fn search_duckduckgo(&self, query: &str) -> ToolResult<Vec<SearchResult>> {
        // Note: This is a simplified implementation.
        // For production, use the official DDG API or a proper library.

        let url = format!(
            "https://html.duckduckgo.com/html/?q={}",
            urlencoding::encode(query)
        );

        let response = self
            .client
            .get(&url)
            .header("User-Agent", "Mozilla/5.0 (compatible; RustBot/1.0)")
            .send()
            .await
            .map_err(|e| ToolError::Execution(format!("Search request failed: {}", e)))?;

        let html = response.text().await.map_err(|e| {
            ToolError::Execution(format!("Failed to read response: {}", e))
        })?;

        // Simple parsing - extract results from HTML
        // In production, use a proper HTML parser like scraper
        let mut results = Vec::new();
        for line in html.lines() {
            if line.contains("result__a") {
                // Extract title and URL from result links
                if let Some(url) = extract_attr(line, "href") {
                    if let Some(title) = extract_text_between(line, ">", "</a>") {
                        results.push(SearchResult {
                            title: html_unescape(title),
                            url,
                            snippet: String::new(),
                        });
                        if results.len() >= self.config.max_results as usize {
                            break;
                        }
                    }
                }
            }
        }

        Ok(results)
    }

    /// Search using Tavily API
    async fn search_tavily(&self, query: &str) -> ToolResult<Vec<SearchResult>> {
        if self.config.api_key.is_empty() {
            return Err(ToolError::Execution(
                "Tavily API key not configured".to_string(),
            ));
        }

        let url = "https://api.tavily.com/search";

        let response = self
            .client
            .post(url)
            .header("Content-Type", "application/json")
            .body(format!(
                r#"{{"api_key": "{}", "query": "{}", "max_results": {}}}"#,
                self.config.api_key,
                query,
                self.config.max_results
            ))
            .send()
            .await
            .map_err(|e| ToolError::Execution(format!("Search request failed: {}", e)))?;

        let data: Value = response.json().await.map_err(|e| {
            ToolError::Execution(format!("Failed to parse response: {}", e))
        })?;

        let results = data
            .get("results")
            .and_then(|r| r.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|item| {
                        Some(SearchResult {
                            title: item.get("title")?.as_str()?.to_string(),
                            url: item.get("url")?.as_str()?.to_string(),
                            snippet: item.get("content")?.as_str()?.to_string(),
                        })
                    })
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();

        Ok(results)
    }

    /// Search using SearXNG
    async fn search_searxng(&self, query: &str) -> ToolResult<Vec<SearchResult>> {
        let base_url = if self.config.base_url.is_empty() {
            "https://searx.be"
        } else {
            &self.config.base_url
        };

        let url = format!(
            "{}/search?q={}&format=json",
            base_url,
            urlencoding::encode(query)
        );

        let response = self
            .client
            .get(&url)
            .header("User-Agent", "Mozilla/5.0 (compatible; RustBot/1.0)")
            .send()
            .await
            .map_err(|e| ToolError::Execution(format!("Search request failed: {}", e)))?;

        let data: Value = response.json().await.map_err(|e| {
            ToolError::Execution(format!("Failed to parse response: {}", e))
        })?;

        let results = data
            .get("results")
            .and_then(|r| r.as_array())
            .map(|arr| {
                arr.iter()
                    .take(self.config.max_results as usize)
                    .filter_map(|item| {
                        Some(SearchResult {
                            title: item.get("title")?.as_str()?.to_string(),
                            url: item.get("url")?.as_str()?.to_string(),
                            snippet: item.get("content").and_then(|c| c.as_str()).unwrap_or("").to_string(),
                        })
                    })
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();

        Ok(results)
    }
}

#[async_trait]
impl Tool for WebSearchTool {
    fn name(&self) -> &str {
        "web_search"
    }

    fn description(&self) -> &str {
        "Search the web for information. Returns search results with titles, URLs, and snippets."
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "query": {
                    "type": "string",
                    "description": "Search query",
                },
            },
            "required": ["query"],
        })
    }

    async fn execute(&self, params: Value) -> ToolResult<Value> {
        let query = params
            .get("query")
            .and_then(|q| q.as_str())
            .ok_or_else(|| ToolError::InvalidParams("Missing 'query' parameter".to_string()))?;

        info!("Web search: {}", query);

        let results = match self.config.provider {
            SearchProvider::Brave => self.search_brave(query).await,
            SearchProvider::Jina => self.search_jina(query).await,
            SearchProvider::Tavily => self.search_tavily(query).await,
            SearchProvider::SearXNG => self.search_searxng(query).await,
            SearchProvider::DuckDuckGo => self.search_duckduckgo(query).await,
        }?;

        let results_json: Vec<Value> = results
            .iter()
            .map(|r| {
                json!({
                    "title": r.title,
                    "url": r.url,
                    "snippet": r.snippet,
                })
            })
            .collect();

        Ok(json!({
            "query": query,
            "results": results_json,
            "count": results.len(),
        }))
    }
}

// ============================================================================
// Web Fetch Tool
// ============================================================================

/// Web fetch tool for retrieving page content
pub struct WebFetchTool {
    client: Client,
    proxy: Option<String>,
}

impl WebFetchTool {
    pub fn new(proxy: Option<String>) -> Self {
        let client = Client::builder()
            .timeout(Duration::from_secs(30))
            .build()
            .expect("Failed to create HTTP client");

        Self { client, proxy }
    }
}

#[async_trait]
impl Tool for WebFetchTool {
    fn name(&self) -> &str {
        "web_fetch"
    }

    fn description(&self) -> &str {
        "Fetch the content of a web page. Returns the text content of the page."
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "url": {
                    "type": "string",
                    "description": "URL of the web page to fetch",
                },
            },
            "required": ["url"],
        })
    }

    async fn execute(&self, params: Value) -> ToolResult<Value> {
        let url = params
            .get("url")
            .and_then(|u| u.as_str())
            .ok_or_else(|| ToolError::InvalidParams("Missing 'url' parameter".to_string()))?;

        info!("Fetching URL: {}", url);

        // Validate URL
        if !url.starts_with("http://") && !url.starts_with("https://") {
            return Err(ToolError::InvalidParams(
                "URL must start with http:// or https://".to_string(),
            ));
        }

        let mut request = self
            .client
            .get(url)
            .header("User-Agent", "Mozilla/5.0 (compatible; RustBot/1.0)");

        // Add proxy if configured
        if let Some(ref proxy) = self.proxy {
            request = request.header("Proxy", proxy);
        }

        let response = request
            .send()
            .await
            .map_err(|e| ToolError::Execution(format!("Request failed: {}", e)))?;

        let status = response.status();
        if !status.is_success() {
            return Err(ToolError::Execution(format!(
                "HTTP error: {}",
                status
            )));
        }

        let content = response.text().await.map_err(|e| {
            ToolError::Execution(format!("Failed to read response: {}", e))
        })?;

        // Truncate if too long
        let truncated = content.len() > 50000;
        let content = if truncated {
            format!("{}...\n[Content truncated]", &content[..50000])
        } else {
            content
        };

        Ok(json!({
            "url": url,
            "content": content,
            "truncated": truncated,
        }))
    }
}

// ============================================================================
// Helper Functions
// ============================================================================

/// Extract attribute value from HTML tag
fn extract_attr(line: &str, attr: &str) -> Option<String> {
    let pattern = format!("{}=\"", attr);
    if let Some(start) = line.find(&pattern) {
        let rest = &line[start + pattern.len()..];
        if let Some(end) = rest.find('"') {
            return Some(rest[..end].to_string());
        }
    }
    None
}

/// Extract text between two delimiters
fn extract_text_between(line: &str, start: &str, end: &str) -> Option<String> {
    if let Some(start_pos) = line.find(start) {
        let rest = &line[start_pos + start.len()..];
        if let Some(end_pos) = rest.find(end) {
            return Some(rest[..end_pos].to_string());
        }
    }
    None
}

/// Simple HTML unescape
fn html_unescape(s: String) -> String {
    s.replace("&quot;", "\"")
        .replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&nbsp;", " ")
}

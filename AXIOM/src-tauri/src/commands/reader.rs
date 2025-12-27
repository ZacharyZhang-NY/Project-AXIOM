use reqwest::redirect::Policy;
use scraper::{Html, Selector};
use serde::Serialize;
use std::time::Duration;

use super::tabs::CommandResult;

#[derive(Debug, Serialize)]
pub struct ReaderExtractResult {
    pub url: String,
    pub title: String,
    pub byline: Option<String>,
    pub content_html: String,
}

#[tauri::command]
pub async fn extract_reader(url: String) -> CommandResult<ReaderExtractResult> {
    let trimmed = url.trim();
    if trimmed.is_empty() {
        return CommandResult::err("URL is empty".to_string());
    }

    let parsed = match url::Url::parse(trimmed) {
        Ok(u) => u,
        Err(e) => return CommandResult::err(e.to_string()),
    };

    if parsed.scheme() != "http" && parsed.scheme() != "https" {
        return CommandResult::err("Reader mode supports only http(s) URLs".to_string());
    }

    let client = match reqwest::Client::builder()
        .redirect(Policy::limited(5))
        .timeout(Duration::from_secs(12))
        .user_agent("Mozilla/5.0 (AXIOM Reader)")
        .build()
    {
        Ok(c) => c,
        Err(e) => return CommandResult::err(e.to_string()),
    };

    let resp = match client.get(parsed).send().await {
        Ok(r) => r,
        Err(e) => return CommandResult::err(e.to_string()),
    };

    if !resp.status().is_success() {
        return CommandResult::err(format!("HTTP {}", resp.status()));
    }

    let final_url = resp.url().to_string();
    let body = match resp.text().await {
        Ok(t) => t,
        Err(e) => return CommandResult::err(e.to_string()),
    };

    let doc = Html::parse_document(&body);

    let title = extract_title(&doc).unwrap_or_else(|| final_url.clone());
    let byline = extract_byline(&doc);
    let content_html = extract_content_html(&doc);

    if content_html.trim().is_empty() {
        return CommandResult::err("No readable content found".to_string());
    }

    CommandResult::ok(ReaderExtractResult {
        url: final_url,
        title,
        byline,
        content_html,
    })
}

fn extract_title(doc: &Html) -> Option<String> {
    let og_title = Selector::parse("meta[property='og:title']").ok()?;
    for el in doc.select(&og_title) {
        if let Some(content) = el.value().attr("content") {
            let cleaned = normalize_whitespace(content);
            if !cleaned.is_empty() {
                return Some(cleaned);
            }
        }
    }

    if let Ok(sel) = Selector::parse("meta[name='twitter:title']") {
        for el in doc.select(&sel) {
            if let Some(content) = el.value().attr("content") {
                let cleaned = normalize_whitespace(content);
                if !cleaned.is_empty() {
                    return Some(cleaned);
                }
            }
        }
    }

    if let Ok(sel) = Selector::parse("title") {
        for el in doc.select(&sel) {
            let text = el.text().collect::<Vec<_>>().join(" ");
            let cleaned = normalize_whitespace(&text);
            if !cleaned.is_empty() {
                return Some(cleaned);
            }
        }
    }

    if let Ok(sel) = Selector::parse("h1") {
        for el in doc.select(&sel) {
            let text = el.text().collect::<Vec<_>>().join(" ");
            let cleaned = normalize_whitespace(&text);
            if cleaned.len() >= 6 {
                return Some(cleaned);
            }
        }
    }

    None
}

fn extract_byline(doc: &Html) -> Option<String> {
    if let Ok(sel) = Selector::parse("meta[name='author']") {
        for el in doc.select(&sel) {
            if let Some(content) = el.value().attr("content") {
                let cleaned = normalize_whitespace(content);
                if !cleaned.is_empty() {
                    return Some(cleaned);
                }
            }
        }
    }

    None
}

fn extract_content_html(doc: &Html) -> String {
    let selectors = [
        ("article", 400usize),
        ("main, [role='main']", 400usize),
        ("body", 0usize),
    ];

    for (selector_str, min_len) in selectors {
        let sel = match Selector::parse(selector_str) {
            Ok(s) => s,
            Err(_) => continue,
        };

        let mut best_score = 0usize;
        let mut best_html = String::new();

        for el in doc.select(&sel) {
            let score = element_text_len(&el);
            if score <= best_score {
                continue;
            }
            let rendered = render_reader_html(&el);
            if rendered.trim().is_empty() {
                continue;
            }
            best_score = score;
            best_html = rendered;
        }

        if best_score >= min_len && !best_html.trim().is_empty() {
            return best_html;
        }
    }

    String::new()
}

fn element_text_len(el: &scraper::ElementRef<'_>) -> usize {
    el.text()
        .map(|t| t.trim())
        .filter(|t| !t.is_empty())
        .map(|t| t.len())
        .sum()
}

fn render_reader_html(root: &scraper::ElementRef<'_>) -> String {
    let block_sel = match Selector::parse("h2, h3, p, blockquote, pre, li") {
        Ok(s) => s,
        Err(_) => return String::new(),
    };

    let mut out = String::new();
    let mut blocks = 0usize;

    for el in root.select(&block_sel) {
        if blocks >= 320 {
            break;
        }

        let tag = el.value().name();
        let text = if tag == "pre" {
            el.text().collect::<Vec<_>>().join("")
        } else {
            el.text().collect::<Vec<_>>().join(" ")
        };

        let cleaned = if tag == "pre" {
            text.trim_end().to_string()
        } else {
            normalize_whitespace(&text)
        };

        if cleaned.is_empty() {
            continue;
        }

        let escaped = escape_html(&cleaned);
        match tag {
            "h2" => {
                out.push_str("<h2>");
                out.push_str(&escaped);
                out.push_str("</h2>\n");
            }
            "h3" => {
                out.push_str("<h3>");
                out.push_str(&escaped);
                out.push_str("</h3>\n");
            }
            "blockquote" => {
                out.push_str("<blockquote>");
                out.push_str(&escaped);
                out.push_str("</blockquote>\n");
            }
            "pre" => {
                out.push_str("<pre><code>");
                out.push_str(&escaped);
                out.push_str("</code></pre>\n");
            }
            "li" => {
                out.push_str("<p>• ");
                out.push_str(&escaped);
                out.push_str("</p>\n");
            }
            _ => {
                if cleaned.len() < 20 {
                    continue;
                }
                out.push_str("<p>");
                out.push_str(&escaped);
                out.push_str("</p>\n");
            }
        }

        blocks += 1;
    }

    out
}

fn normalize_whitespace(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut last_space = false;
    for ch in input.chars() {
        if ch.is_whitespace() {
            if !last_space {
                out.push(' ');
                last_space = true;
            }
        } else {
            out.push(ch);
            last_space = false;
        }
    }
    out.trim().to_string()
}

fn escape_html(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    for ch in input.chars() {
        match ch {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#39;"),
            _ => out.push(ch),
        }
    }
    out
}

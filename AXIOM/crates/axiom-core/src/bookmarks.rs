use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Bookmark {
    pub title: String,
    pub url: String,
    #[serde(default)]
    pub folder: Option<String>,
}

pub fn normalize_folder(folder: Option<String>) -> Option<String> {
    folder
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

pub fn folders_from_bookmarks(bookmarks: &[Bookmark]) -> Vec<String> {
    let mut set = BTreeSet::new();
    for bookmark in bookmarks {
        if let Some(folder) = bookmark.folder.as_deref() {
            let folder = folder.trim();
            if !folder.is_empty() {
                set.insert(folder.to_string());
            }
        }
    }
    set.into_iter().collect()
}

fn escape_html(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for ch in s.chars() {
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

#[derive(Default)]
struct FolderNode {
    folders: BTreeMap<String, FolderNode>,
    bookmarks: Vec<Bookmark>,
}

fn insert_bookmark(root: &mut FolderNode, bookmark: Bookmark) {
    let mut node = root;
    if let Some(folder) = bookmark.folder.as_deref() {
        for part in folder.split('/').map(str::trim).filter(|s| !s.is_empty()) {
            node = node.folders.entry(part.to_string()).or_default();
        }
    }
    node.bookmarks.push(bookmark);
}

pub fn export_bookmarks_html(bookmarks: &[Bookmark]) -> String {
    let mut root = FolderNode::default();
    for bookmark in bookmarks.iter().cloned() {
        insert_bookmark(&mut root, bookmark);
    }

    fn render_node(node: &FolderNode, out: &mut String, indent: usize) {
        let pad = |out: &mut String, n: usize| {
            for _ in 0..n {
                out.push(' ');
            }
        };

        for (name, child) in &node.folders {
            pad(out, indent);
            out.push_str("<DT><H3>");
            out.push_str(&escape_html(name));
            out.push_str("</H3>\n");

            pad(out, indent);
            out.push_str("<DL><p>\n");
            render_node(child, out, indent + 2);
            pad(out, indent);
            out.push_str("</DL><p>\n");
        }

        let mut items = node.bookmarks.clone();
        items.sort_by(|a, b| a.title.cmp(&b.title).then_with(|| a.url.cmp(&b.url)));
        for bookmark in items {
            pad(out, indent);
            out.push_str("<DT><A HREF=\"");
            out.push_str(&escape_html(&bookmark.url));
            out.push_str("\">");
            out.push_str(&escape_html(&bookmark.title));
            out.push_str("</A>\n");
        }
    }

    let mut out = String::new();
    out.push_str("<!DOCTYPE NETSCAPE-Bookmark-file-1>\n");
    out.push_str("<META HTTP-EQUIV=\"Content-Type\" CONTENT=\"text/html; charset=UTF-8\">\n");
    out.push_str("<TITLE>Bookmarks</TITLE>\n");
    out.push_str("<H1>Bookmarks</H1>\n");
    out.push_str("<DL><p>\n");
    render_node(&root, &mut out, 2);
    out.push_str("</DL><p>\n");
    out
}

fn find_from(haystack: &str, needle: &str, start: usize) -> Option<usize> {
    haystack.get(start..)?.find(needle).map(|i| start + i)
}

fn extract_attr(tag_lower: &str, tag_raw: &str, attr: &str) -> Option<String> {
    let attr = format!("{attr}=");
    let idx = tag_lower.find(&attr)?;
    let mut i = idx + attr.len();
    let bytes = tag_lower.as_bytes();
    if i >= bytes.len() {
        return None;
    }

    let quote = bytes[i] as char;
    if quote == '"' || quote == '\'' {
        i += 1;
        let end = tag_lower.get(i..)?.find(quote).map(|j| i + j)?;
        return Some(tag_raw.get(i..end)?.to_string());
    }

    let end = tag_lower
        .get(i..)?
        .find(|c: char| c.is_whitespace() || c == '>')
        .map(|j| i + j)
        .unwrap_or(tag_lower.len());
    Some(tag_raw.get(i..end)?.to_string())
}

pub fn import_bookmarks_html(html: &str) -> Vec<Bookmark> {
    let lower = html.to_ascii_lowercase();
    let mut pos = 0usize;
    let mut stack: Vec<String> = Vec::new();
    let mut pending_folder: Option<String> = None;
    let mut bookmarks = Vec::new();

    while pos < lower.len() {
        if let Some(h3) = find_from(&lower, "<h3", pos) {
            let gt = match find_from(&lower, ">", h3) {
                Some(i) => i,
                None => break,
            };
            let start = gt + 1;
            let end = match find_from(&lower, "</h3", start) {
                Some(i) => i,
                None => break,
            };
            let name = html.get(start..end).unwrap_or("").trim();
            pending_folder = (!name.is_empty()).then(|| name.to_string());
            pos = end;
            continue;
        }

        if let Some(dl) = find_from(&lower, "<dl", pos) {
            if let Some(folder) = pending_folder.take() {
                stack.push(folder);
            }
            pos = dl + 3;
            continue;
        }

        if let Some(end_dl) = find_from(&lower, "</dl", pos) {
            if !stack.is_empty() {
                stack.pop();
            }
            pos = end_dl + 4;
            continue;
        }

        if let Some(a) = find_from(&lower, "<a", pos) {
            let gt = match find_from(&lower, ">", a) {
                Some(i) => i,
                None => break,
            };
            let tag_lower = lower.get(a..gt).unwrap_or("");
            let tag_raw = html.get(a..gt).unwrap_or("");
            let url = extract_attr(tag_lower, tag_raw, "href").unwrap_or_default();

            let text_start = gt + 1;
            let text_end = match find_from(&lower, "</a", text_start) {
                Some(i) => i,
                None => break,
            };
            let title = html
                .get(text_start..text_end)
                .unwrap_or("")
                .trim()
                .to_string();
            let folder = if stack.is_empty() {
                None
            } else {
                Some(stack.join("/"))
            };

            if !url.trim().is_empty() {
                bookmarks.push(Bookmark {
                    title: if title.is_empty() { url.clone() } else { title },
                    url,
                    folder,
                });
            }

            pos = text_end + 4;
            continue;
        }

        pos += 1;
    }

    bookmarks
}

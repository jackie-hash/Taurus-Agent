//! MEMORY.md index parsing and management.
//!
//! The index file follows Hermes' format:
//!
//! ```markdown
//! # 记忆系统
//!
//! 本文件为系统入口——每次会话自动加载。详细知识见 [[index.md]]。
//!
//! ## 快速导航
//!
//! - [[user_role.md]] — 用户角色与偏好
//! - [[feedback_testing.md]] — 测试反馈
//!
//! 完整索引 → [[index.md]]
//! ```
//!
//! Each line is a wikilink-style reference: `- [[file.md]] — description`.
//! The `[[file.md]]` form is used for memory files; standard Markdown
//! `[title](file.md)` is also accepted for compatibility.

use std::collections::BTreeMap;
use std::io;

use crate::types::{IndexEntry, MemoryIndex};

/// Parse a `MEMORY.md` string into a structured `MemoryIndex`.
///
/// Recognises both wikilink (`[[file.md]]`) and standard Markdown link
/// (`[title](file.md)`) formats. Lines without a recognised link pattern
/// are treated as preamble.
#[must_use]
pub fn parse_index(content: &str) -> MemoryIndex {
    let mut entries = Vec::new();
    let mut preamble = Vec::new();

    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || (trimmed.starts_with('#') && !trimmed.contains("[[")) {
            preamble.push(line.to_string());
            continue;
        }

        // Only parse list items as potential entries.
        let is_list_item = trimmed.starts_with("- ") || trimmed.starts_with("* ");
        if !is_list_item {
            preamble.push(line.to_string());
            continue;
        }

        // Try wikilink format: [[file.md]]
        if let Some(entry) = parse_wikilink_line(trimmed) {
            entries.push(entry);
            continue;
        }

        // Try Markdown link format: [title](file.md)
        if let Some(entry) = parse_mdlink_line(trimmed) {
            entries.push(entry);
            continue;
        }

        // List item without a recognised link — keep as preamble.
        preamble.push(line.to_string());
    }

    MemoryIndex { entries, preamble }
}

/// Render a `MemoryIndex` back to a `MEMORY.md` string.
#[must_use]
pub fn render_index(index: &MemoryIndex) -> String {
    let mut lines: Vec<String> = index.preamble.clone();

    if !index.entries.is_empty() {
        if !lines.is_empty() {
            lines.push(String::new());
        }
        lines.push("## 快速导航".to_string());
        lines.push(String::new());
        for entry in &index.entries {
            lines.push(format!(
                "- [[{file}]] — {desc}",
                file = entry.file,
                desc = entry.description
            ));
        }
        lines.push(String::new());
        lines.push("完整索引 → [[index.md]]".to_string());
    }

    lines.join("\n")
}

/// Group entries by memory type inferred from the file path.
///
/// - Files in `project/<slug>/` → Project
/// - Files in `reference/` → Reference
/// - Files with `feedback` in the name → Feedback
/// - Everything else → User
#[must_use]
pub fn group_by_type(entries: &[IndexEntry]) -> BTreeMap<String, Vec<&IndexEntry>> {
    let mut groups: BTreeMap<String, Vec<&IndexEntry>> = BTreeMap::new();
    for entry in entries {
        let category = categorize_file(&entry.file);
        groups.entry(category).or_default().push(entry);
    }
    groups
}

fn categorize_file(file: &str) -> String {
    if file.starts_with("project/") {
        "project".to_string()
    } else if file.starts_with("reference/") {
        "reference".to_string()
    } else if file.contains("feedback") {
        "feedback".to_string()
    } else {
        "user".to_string()
    }
}

/// Add an entry to the index, or update it if one with the same file already exists.
pub fn upsert_entry(index: &mut MemoryIndex, entry: IndexEntry) {
    if let Some(existing) = index.entries.iter_mut().find(|e| e.file == entry.file) {
        existing.title = entry.title;
        existing.description = entry.description;
    } else {
        index.entries.push(entry);
    }
}

/// Remove an entry from the index by filename.
pub fn remove_entry(index: &mut MemoryIndex, file: &str) -> bool {
    let len_before = index.entries.len();
    index.entries.retain(|e| e.file != file);
    index.entries.len() < len_before
}

// ---------------------------------------------------------------------------
// Line parsers
// ---------------------------------------------------------------------------

/// Parse `- [[file.md]] — description` or `- [[file.md]]: description`
fn parse_wikilink_line(line: &str) -> Option<IndexEntry> {
    let inner = line.trim_start_matches(['-', '*', ' ']).trim();
    let start = inner.find("[[")?;
    let end = inner[start..].find("]]")?;
    let file = &inner[start + 2..start + end];
    let after = inner[start + end + 2..].trim();
    // Strip leading em-dash, colon, or space
    let desc = after
        .trim_start_matches(['—', ':', '-', ' '])
        .trim();
    Some(IndexEntry {
        title: file.trim_end_matches(".md").to_string(),
        file: file.to_string(),
        description: desc.to_string(),
    })
}

/// Parse `- [title](file.md) — description`
fn parse_mdlink_line(line: &str) -> Option<IndexEntry> {
    let inner = line.trim_start_matches(['-', '*', ' ']).trim();
    let title_start = inner.find('[')?;
    let title_end = inner[title_start..].find(']')?;
    let title = &inner[title_start + 1..title_start + title_end];

    let paren_start = inner[title_start + title_end..].find('(')?;
    let paren_end = inner[title_start + title_end + paren_start..].find(')')?;
    let file = &inner[title_start + title_end + paren_start + 1..title_start + title_end + paren_start + paren_end];

    let after = inner[title_start + title_end + paren_start + paren_end + 1..].trim();
    let desc = after
        .trim_start_matches(['—', ':', '-', ' '])
        .trim();

    Some(IndexEntry {
        title: title.to_string(),
        file: file.to_string(),
        description: desc.to_string(),
    })
}

// ---------------------------------------------------------------------------
// Error type
// ---------------------------------------------------------------------------

/// Errors specific to index operations.
#[derive(Debug)]
pub enum IndexError {
    /// Index file not found at the given path.
    NotFound(String),
    /// Duplicate entry for the same file.
    DuplicateFile(String),
    /// I/O error during index read/write.
    Io(io::Error),
}

impl std::fmt::Display for IndexError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotFound(path) => write!(f, "index file not found: {path}"),
            Self::DuplicateFile(file) => write!(f, "duplicate entry for file: {file}"),
            Self::Io(e) => write!(f, "I/O error: {e}"),
        }
    }
}

impl std::error::Error for IndexError {}

impl From<io::Error> for IndexError {
    fn from(value: io::Error) -> Self {
        Self::Io(value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_index_wikilinks() {
        let content = "# 记忆系统\n\n## 快速导航\n\n- [[user_role.md]] — 用户角色与偏好\n- [[feedback_testing.md]] — 测试反馈\n\n完整索引 → [[index.md]]";
        let index = parse_index(content);
        assert_eq!(index.entries.len(), 2);
        assert_eq!(index.entries[0].file, "user_role.md");
        assert_eq!(index.entries[0].description, "用户角色与偏好");
        assert_eq!(index.entries[1].file, "feedback_testing.md");
    }

    #[test]
    fn test_parse_index_mdlinks() {
        let content = "- [User Role](user_role.md) — user preferences\n- [Feedback](feedback.md) — testing feedback";
        let index = parse_index(content);
        assert_eq!(index.entries.len(), 2);
        assert_eq!(index.entries[0].title, "User Role");
        assert_eq!(index.entries[0].file, "user_role.md");
    }

    #[test]
    fn test_upsert_entry() {
        let mut index = MemoryIndex::default();
        upsert_entry(
            &mut index,
            IndexEntry {
                title: "test".into(),
                file: "test.md".into(),
                description: "first".into(),
            },
        );
        assert_eq!(index.entries.len(), 1);
        upsert_entry(
            &mut index,
            IndexEntry {
                title: "test".into(),
                file: "test.md".into(),
                description: "updated".into(),
            },
        );
        assert_eq!(index.entries.len(), 1);
        assert_eq!(index.entries[0].description, "updated");
    }

    #[test]
    fn test_remove_entry() {
        let mut index = MemoryIndex::default();
        index.entries.push(IndexEntry {
            title: "a".into(),
            file: "a.md".into(),
            description: "".into(),
        });
        assert!(remove_entry(&mut index, "a.md"));
        assert!(index.entries.is_empty());
        assert!(!remove_entry(&mut index, "nonexistent.md"));
    }

    #[test]
    fn test_render_index_roundtrip() {
        let content = "# 记忆系统\n\n## 快速导航\n\n- [[user_role.md]] — 用户角色\n";
        let index = parse_index(content);
        let rendered = render_index(&index);
        assert!(rendered.contains("[[user_role.md]]"));
    }
}

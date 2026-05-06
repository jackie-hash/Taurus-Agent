//! Core types for the Hermes-style persistent memory system.

use std::fmt;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

/// The four memory categories.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MemoryType {
    User,
    Project,
    Feedback,
    Reference,
}

impl MemoryType {
    pub fn parse(s: &str) -> Option<Self> {
        match s.trim().to_lowercase().as_str() {
            "user" => Some(Self::User),
            "project" => Some(Self::Project),
            "feedback" => Some(Self::Feedback),
            "reference" => Some(Self::Reference),
            _ => None,
        }
    }

    #[must_use]
    pub fn dir_name(self) -> &'static str {
        match self {
            Self::User | Self::Feedback => "",
            Self::Project => "project",
            Self::Reference => "reference",
        }
    }
}

impl fmt::Display for MemoryType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            Self::User => "user",
            Self::Project => "project",
            Self::Feedback => "feedback",
            Self::Reference => "reference",
        };
        write!(f, "{s}")
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryFrontmatter {
    pub name: String,
    pub description: String,
    #[serde(rename = "type")]
    pub memory_type: MemoryType,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryEntry {
    pub frontmatter: MemoryFrontmatter,
    pub body: String,
    pub file_path: PathBuf,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndexEntry {
    pub title: String,
    pub file: String,
    pub description: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct MemoryIndex {
    pub entries: Vec<IndexEntry>,
    pub preamble: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct MemoryBlock {
    pub content: String,
    pub file_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CaptureSuggestion {
    pub memory_type: MemoryType,
    pub name: String,
    pub description: String,
    pub content: String,
    pub rationale: String,
}

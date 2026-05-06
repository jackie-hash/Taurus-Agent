//! LLM-based auto-memory capture from conversation.
//!
//! At session end, the last N turns of conversation are fed to an LLM with a
//! structured prompt that asks it to identify durable memories worth saving.
//! The LLM returns a JSON array of `CaptureSuggestion` values, which are then
//! written to the memory store.

use std::io;

use serde::Deserialize;

use crate::store::MemoryStore;
use crate::types::CaptureSuggestion;
#[cfg(test)]
use crate::types::MemoryType;

/// Maximum number of conversation turns to include in the capture prompt.
const MAX_CAPTURE_TURNS: usize = 20;

/// Maximum characters per turn included in the capture prompt.
const MAX_TURN_CHARS: usize = 4000;

/// The system prompt sent to the LLM for memory capture.
const CAPTURE_SYSTEM_PROMPT: &str = r#"You are a memory curator. Analyze the conversation and identify durable information worth preserving across sessions.

## What to capture

You have four memory types available:

- **user**: Information about the user's role, goals, responsibilities, and knowledge.
  Example: "user is a data scientist focused on observability"

- **project**: Facts about ongoing work, goals, initiatives, bugs, or incidents within a project.
  Example: "merge freeze begins 2026-03-05 for mobile release cut"

- **feedback**: Guidance the user has given about how to approach work — what to avoid and what to keep doing.
  Example: "integration tests must hit a real database, not mocks"

- **reference**: Pointers to where information can be found in external systems.
  Example: "pipeline bugs are tracked in Linear project INGEST"

## What NOT to capture

- Code patterns, conventions, or file paths (these are visible in the repo)
- Git history or recent changes (git log is authoritative)
- Debugging solutions or fix recipes (the fix is in the code)
- Anything already in CLAUDE.md files
- Ephemeral task details or in-progress work

## Instructions

1. Review the conversation below.
2. Identify 0-3 durable memories worth saving. Return an empty array if nothing is worth keeping.
3. For each memory, provide a concise name, description, type, content, and rationale.

Return your response as a JSON object with a single key "memories" containing an array:

```json
{
  "memories": [
    {
      "memory_type": "user",
      "name": "user_role",
      "description": "User is a backend engineer working on API performance",
      "content": "The user is a backend engineer focused on API performance optimization. They prefer Rust and have deep knowledge of database internals.",
      "rationale": "Knowing the user's role helps tailor future responses"
    }
  ]
}
```

Keep each memory body under 500 characters. Be selective — only capture what would genuinely help in future conversations."#;

/// Stateful memory capture session.
///
/// Accumulates conversation turns during a session, then at session end
/// constructs a prompt and parses the LLM response to persist new memories.
#[derive(Debug)]
pub struct MemoryCapture {
    /// Accumulated conversation turns as (role, content) pairs.
    turns: Vec<(String, String)>,
    /// The memory store to write captured memories into.
    #[allow(dead_code)]
    store: MemoryStore,
}

impl MemoryCapture {
    /// Create a new capture session backed by the given store.
    #[must_use]
    pub fn new(store: MemoryStore) -> Self {
        Self {
            turns: Vec::new(),
            store,
        }
    }

    /// Record a conversation turn.
    pub fn record_turn(&mut self, role: &str, content: &str) {
        // Truncate very long messages to keep the capture prompt manageable.
        let truncated = if content.len() > MAX_TURN_CHARS {
            format!("{}…(truncated)", &content[..MAX_TURN_CHARS])
        } else {
            content.to_string()
        };
        self.turns.push((role.to_string(), truncated));

        // Keep only the most recent N turns.
        if self.turns.len() > MAX_CAPTURE_TURNS {
            let excess = self.turns.len() - MAX_CAPTURE_TURNS;
            self.turns.drain(0..excess);
        }
    }

    /// Number of recorded turns.
    #[must_use]
    pub fn turn_count(&self) -> usize {
        self.turns.len()
    }

    /// Whether there are enough turns to warrant a capture attempt.
    #[must_use]
    pub fn should_capture(&self) -> bool {
        self.turns.len() >= 3
    }

    /// Build the user prompt for the memory capture LLM call.
    ///
    /// Returns `None` if there aren't enough turns to justify capture.
    #[must_use]
    pub fn build_capture_prompt(&self) -> Option<String> {
        if !self.should_capture() {
            return None;
        }

        let mut prompt = String::from(
            "## Conversation to analyze\n\nBelow is a conversation between a user and an AI assistant. Identify durable memories worth saving.\n\n",
        );

        for (role, content) in &self.turns {
            prompt.push_str(&format!("**{role}**: {content}\n\n"));
        }

        prompt.push_str("## Task\n\nAnalyze the conversation above and return memories in the JSON format specified.");
        Some(prompt)
    }

    /// Return the system prompt for the capture LLM.
    #[must_use]
    pub fn system_prompt() -> &'static str {
        CAPTURE_SYSTEM_PROMPT
    }

    /// Parse the LLM's JSON response into a list of capture suggestions.
    ///
    /// Accepts raw LLM output which may contain markdown code fences.
    pub fn parse_response(response: &str) -> Result<Vec<CaptureSuggestion>, CaptureError> {
        let json_str = extract_json(response)?;

        #[derive(Deserialize)]
        struct CaptureResponse {
            memories: Vec<CaptureSuggestion>,
        }

        let parsed: CaptureResponse = serde_json::from_str(&json_str)?;
        Ok(parsed.memories)
    }

    /// Consume the capture session and write all suggestions to the store.
    pub fn persist_suggestions(
        self,
        store: &MemoryStore,
        suggestions: &[CaptureSuggestion],
        project_slug: Option<&str>,
    ) -> Vec<io::Result<String>> {
        suggestions
            .iter()
            .map(|s| {
                let path = store.write_entry(
                    s.memory_type,
                    &s.name,
                    &s.description,
                    &s.content,
                    project_slug,
                )?;
                Ok(path.display().to_string())
            })
            .collect()
    }
}

// ---------------------------------------------------------------------------
// JSON extraction (handles markdown code fences)
// ---------------------------------------------------------------------------

/// Extract a JSON object from LLM output that may be wrapped in markdown fences.
fn extract_json(raw: &str) -> Result<String, CaptureError> {
    let trimmed = raw.trim();

    // Try to extract from ```json ... ``` fence
    if let Some(inner) = extract_fenced(trimmed, "```json") {
        return Ok(inner);
    }
    // Try ``` ... ``` without language tag
    if let Some(inner) = extract_fenced(trimmed, "```") {
        // Only use if it looks like JSON
        let candidate = inner.trim();
        if candidate.starts_with('{') || candidate.starts_with('[') {
            return Ok(candidate.to_string());
        }
    }

    // Assume raw JSON
    if trimmed.starts_with('{') || trimmed.starts_with('[') {
        return Ok(trimmed.to_string());
    }

    Err(CaptureError::NoJsonFound)
}

fn extract_fenced(text: &str, fence: &str) -> Option<String> {
    let start_marker = text.find(fence)?;
    let after_start = &text[start_marker + fence.len()..];
    // Skip to end of opening line
    let after_newline = after_start.find('\n')?;
    let body_start = start_marker + fence.len() + after_newline + 1;
    let closing = text[body_start..].find(fence)?;
    Some(text[body_start..body_start + closing].to_string())
}

#[derive(Debug)]
pub enum CaptureError {
    NoJsonFound,
    Parse(serde_json::Error),
}

impl std::fmt::Display for CaptureError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NoJsonFound => write!(f, "no JSON found in LLM response"),
            Self::Parse(e) => write!(f, "JSON parse error: {e}"),
        }
    }
}

impl std::error::Error for CaptureError {}

impl From<serde_json::Error> for CaptureError {
    fn from(value: serde_json::Error) -> Self {
        Self::Parse(value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_empty_capture() {
        let store = MemoryStore::new(std::path::PathBuf::from("/tmp/test"));
        let capture = MemoryCapture::new(store);
        assert!(!capture.should_capture());
        assert!(capture.build_capture_prompt().is_none());
    }

    #[test]
    fn test_capture_with_turns() {
        let store = MemoryStore::new(std::path::PathBuf::from("/tmp/test"));
        let mut capture = MemoryCapture::new(store);
        capture.record_turn("user", "I'm a backend engineer.");
        capture.record_turn("assistant", "Got it, I'll keep that in mind.");
        capture.record_turn("user", "Please optimize the database queries.");
        assert!(capture.should_capture());
        let prompt = capture.build_capture_prompt().unwrap();
        assert!(prompt.contains("backend engineer"));
    }

    #[test]
    fn test_turn_truncation() {
        let store = MemoryStore::new(std::path::PathBuf::from("/tmp/test"));
        let mut capture = MemoryCapture::new(store);
        let long = "x".repeat(5000);
        capture.record_turn("user", &long);
        // Should be truncated to MAX_TURN_CHARS + "(truncated)" marker
        assert!(capture.turns[0].1.len() < 5000);
    }

    #[test]
    fn test_parse_json_response() {
        let resp = r#"```json
{
  "memories": [
    {
      "memory_type": "user",
      "name": "test",
      "description": "A test memory",
      "content": "Test content.",
      "rationale": "For testing"
    }
  ]
}
```"#;
        let suggestions = MemoryCapture::parse_response(resp).unwrap();
        assert_eq!(suggestions.len(), 1);
        assert_eq!(suggestions[0].name, "test");
        assert_eq!(suggestions[0].memory_type, MemoryType::User);
    }

    #[test]
    fn test_parse_empty_memories() {
        let resp = r#"{"memories": []}"#;
        let suggestions = MemoryCapture::parse_response(resp).unwrap();
        assert!(suggestions.is_empty());
    }

    #[test]
    fn test_parse_no_json_errors() {
        let resp = "Just some text, no JSON here.";
        let err = MemoryCapture::parse_response(resp).unwrap_err();
        assert!(matches!(err, CaptureError::NoJsonFound));
    }
}

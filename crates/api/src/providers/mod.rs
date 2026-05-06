//! Provider registry for DeepSeek Taurine.
//!
//! DeepSeek speaks an OpenAI-compatible REST shape, so we route every model
//! through [`openai_compat::OpenAiCompatClient`]. The registry just tells the
//! client which base URL, auth env var, and token limits to use.

use std::future::Future;
use std::pin::Pin;

use serde::Serialize;

use crate::error::ApiError;
use crate::types::{MessageRequest, MessageResponse};

pub mod openai_compat;

#[allow(dead_code)]
pub type ProviderFuture<'a, T> = Pin<Box<dyn Future<Output = Result<T, ApiError>> + Send + 'a>>;

#[allow(dead_code)]
pub trait Provider {
    type Stream;

    fn send_message<'a>(
        &'a self,
        request: &'a MessageRequest,
    ) -> ProviderFuture<'a, MessageResponse>;

    fn stream_message<'a>(
        &'a self,
        request: &'a MessageRequest,
    ) -> ProviderFuture<'a, Self::Stream>;
}

// ---------------------------------------------------------------------------
// Provider kind — only DeepSeek for now
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProviderKind {
    Deepseek,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProviderMetadata {
    pub provider: ProviderKind,
    pub auth_env: &'static str,
    pub base_url_env: &'static str,
    pub default_base_url: &'static str,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ModelTokenLimit {
    pub max_output_tokens: u32,
    pub context_window_tokens: u32,
}

// ---------------------------------------------------------------------------
// Model registry — DeepSeek V4 family
// ---------------------------------------------------------------------------

const MODEL_REGISTRY: &[(&str, ProviderMetadata)] = &[
    (
        "pro",
        ProviderMetadata {
            provider: ProviderKind::Deepseek,
            auth_env: "DEEPSEEK_API_KEY",
            base_url_env: "DEEPSEEK_BASE_URL",
            default_base_url: openai_compat::DEFAULT_DEEPSEEK_BASE_URL,
        },
    ),
    (
        "flash",
        ProviderMetadata {
            provider: ProviderKind::Deepseek,
            auth_env: "DEEPSEEK_API_KEY",
            base_url_env: "DEEPSEEK_BASE_URL",
            default_base_url: openai_compat::DEFAULT_DEEPSEEK_BASE_URL,
        },
    ),
    (
        "deepseek-v4-pro",
        ProviderMetadata {
            provider: ProviderKind::Deepseek,
            auth_env: "DEEPSEEK_API_KEY",
            base_url_env: "DEEPSEEK_BASE_URL",
            default_base_url: openai_compat::DEFAULT_DEEPSEEK_BASE_URL,
        },
    ),
    (
        "deepseek-v4-flash",
        ProviderMetadata {
            provider: ProviderKind::Deepseek,
            auth_env: "DEEPSEEK_API_KEY",
            base_url_env: "DEEPSEEK_BASE_URL",
            default_base_url: openai_compat::DEFAULT_DEEPSEEK_BASE_URL,
        },
    ),
];

// ---------------------------------------------------------------------------
// Model resolution
// ---------------------------------------------------------------------------

/// Resolve short aliases to canonical DeepSeek model IDs.
#[must_use]
pub fn resolve_model_alias(model: &str) -> String {
    let trimmed = model.trim();
    let lower = trimmed.to_ascii_lowercase();
    match lower.as_str() {
        "pro" => "deepseek-v4-pro",
        "flash" => "deepseek-v4-flash",
        "deepseek" | "auto" => "deepseek-v4-flash", // auto routes to flash for the routing call
        _ => trimmed,
    }
    .to_string()
}

/// Return provider metadata for a model name, or `None` if unrecognized.
#[must_use]
pub fn metadata_for_model(model: &str) -> Option<ProviderMetadata> {
    let canonical = resolve_model_alias(model);
    let base_model = canonical.rsplit('/').next().unwrap_or(&canonical);
    if base_model.starts_with("deepseek") {
        return Some(ProviderMetadata {
            provider: ProviderKind::Deepseek,
            auth_env: "DEEPSEEK_API_KEY",
            base_url_env: "DEEPSEEK_BASE_URL",
            default_base_url: openai_compat::DEFAULT_DEEPSEEK_BASE_URL,
        });
    }
    // Fallback: if DEEPSEEK_API_KEY or DEEPSEEK_BASE_URL is set, treat as DeepSeek.
    if std::env::var_os("DEEPSEEK_API_KEY").is_some()
        || std::env::var_os("DEEPSEEK_BASE_URL").is_some()
    {
        return Some(ProviderMetadata {
            provider: ProviderKind::Deepseek,
            auth_env: "DEEPSEEK_API_KEY",
            base_url_env: "DEEPSEEK_BASE_URL",
            default_base_url: openai_compat::DEFAULT_DEEPSEEK_BASE_URL,
        });
    }
    None
}

/// Detect the provider for a given model name. Always returns [`ProviderKind::Deepseek`]
/// when DeepSeek credentials are available.
#[must_use]
pub fn detect_provider_kind(model: &str) -> ProviderKind {
    if let Some(metadata) = metadata_for_model(model) {
        return metadata.provider;
    }
    ProviderKind::Deepseek // default: assume DeepSeek
}

#[must_use]
pub const fn model_family_identity_for_kind(_kind: ProviderKind) -> runtime::ModelFamilyIdentity {
    runtime::ModelFamilyIdentity::DeepSeek
}

#[must_use]
pub fn model_family_identity_for(model: &str) -> runtime::ModelFamilyIdentity {
    model_family_identity_for_kind(detect_provider_kind(model))
}

// ---------------------------------------------------------------------------
// Token limits
// ---------------------------------------------------------------------------

/// Max output tokens for a model. Defaults to 128K for V4.
#[must_use]
pub fn max_tokens_for_model(model: &str) -> u32 {
    model_token_limit(model)
        .map(|limit| limit.max_output_tokens)
        .unwrap_or(128_000)
}

/// Returns the effective max output tokens, preferring a plugin override.
#[must_use]
pub fn max_tokens_for_model_with_override(model: &str, plugin_override: Option<u32>) -> u32 {
    plugin_override.unwrap_or_else(|| max_tokens_for_model(model))
}

/// Hard-coded token limits for known DeepSeek models.
#[must_use]
pub fn model_token_limit(model: &str) -> Option<ModelTokenLimit> {
    let canonical = resolve_model_alias(model);
    let base_model = canonical.rsplit('/').next().unwrap_or(&canonical);
    if base_model.starts_with("deepseek") {
        let (output, context) = if base_model.contains("v4") {
            (128_000u32, 1_000_000u32) // V4: 128K output, 1M context
        } else {
            (32_000u32, 128_000u32) // Legacy DeepSeek
        };
        return Some(ModelTokenLimit {
            max_output_tokens: output,
            context_window_tokens: context,
        });
    }
    // Unrecognized model — still assume DeepSeek V4 limits
    Some(ModelTokenLimit {
        max_output_tokens: 128_000,
        context_window_tokens: 1_000_000,
    })
}

// ---------------------------------------------------------------------------
// Request preflight (context window guard)
// ---------------------------------------------------------------------------

pub fn preflight_message_request(request: &MessageRequest) -> Result<(), ApiError> {
    let Some(limit) = model_token_limit(&request.model) else {
        return Ok(());
    };
    let estimated_input_tokens = estimate_message_request_input_tokens(request);
    let estimated_total_tokens = estimated_input_tokens.saturating_add(request.max_tokens);
    if estimated_total_tokens > limit.context_window_tokens {
        return Err(ApiError::ContextWindowExceeded {
            model: resolve_model_alias(&request.model),
            estimated_input_tokens,
            requested_output_tokens: request.max_tokens,
            estimated_total_tokens,
            context_window_tokens: limit.context_window_tokens,
        });
    }
    Ok(())
}

fn estimate_message_request_input_tokens(request: &MessageRequest) -> u32 {
    let system_chars = request.system.as_deref().unwrap_or("").chars().count() as u32;
    let message_chars: u32 = request
        .messages
        .iter()
        .flat_map(|m| {
            m.content.iter().map(|block| match block {
                crate::types::InputContentBlock::Text { text } => text.chars().count() as u32,
                crate::types::InputContentBlock::Thinking { thinking, .. } => {
                    thinking.chars().count() as u32
                }
                crate::types::InputContentBlock::ToolUse { input, .. } => {
                    serde_json::to_string(input).map_or(0, |s| s.chars().count() as u32)
                }
                crate::types::InputContentBlock::ToolResult {
                    tool_use_id,
                    content,
                    ..
                } => {
                    let content_chars: u32 = content
                        .iter()
                        .map(|c| match c {
                            crate::types::ToolResultContentBlock::Text { text } => {
                                text.chars().count() as u32
                            }
                            crate::types::ToolResultContentBlock::Json { value } => {
                                serde_json::to_string(value).map_or(0, |s| s.chars().count() as u32)
                            }
                        })
                        .sum();
                    tool_use_id.chars().count() as u32 + content_chars
                }
            })
        })
        .sum();
    // Rough heuristic: 4 chars ≈ 1 token for CJK-friendly text
    (system_chars + message_chars) / 4
}

impl Serialize for ProviderKind {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self {
            Self::Deepseek => serializer.serialize_str("deepseek"),
        }
    }
}

// ---------------------------------------------------------------------------
// .env file fallback — reads API keys from .env files when not in environment
// ---------------------------------------------------------------------------

/// Try to read a key from `.env` files in common locations.
/// Returns `None` if the key is not found in any `.env` file.
pub(crate) fn dotenv_value(key: &str) -> Option<String> {
    // Check common .env locations: current dir, home dir
    let candidates: [Option<std::path::PathBuf>; 2] = [
        std::env::current_dir().ok().map(|p| p.join(".env")),
        std::env::var_os("HOME").map(std::path::PathBuf::from).map(|p| p.join(".env")),
    ];
    for path in candidates.into_iter().flatten() {
        if path.exists() {
            if let Some(value) = load_dotenv_file(&path, key) {
                return Some(value);
            }
        }
    }
    None
}

/// Parse a single `.env` file and return the value for `key`, if present.
fn load_dotenv_file(path: &std::path::Path, key: &str) -> Option<String> {
    let contents = std::fs::read_to_string(path).ok()?;
    for line in contents.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        if let Some((k, v)) = trimmed.split_once('=') {
            let k = k.trim();
            let v = v.trim().trim_matches('"').trim_matches('\'');
            if k == key && !v.is_empty() {
                return Some(v.to_string());
            }
        }
    }
    None
}

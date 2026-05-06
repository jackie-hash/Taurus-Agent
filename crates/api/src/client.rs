use crate::error::ApiError;
use crate::providers::openai_compat::{self, OpenAiCompatClient, OpenAiCompatConfig};
use crate::providers::{self, ProviderKind};
use crate::types::{MessageRequest, MessageResponse, StreamEvent};

/// Unified client for DeepSeek API (OpenAI-compatible).
/// Only one variant for now; kept as enum so adding providers later is
/// a one-line change.
#[derive(Debug, Clone)]
pub enum ProviderClient {
    Deepseek(OpenAiCompatClient),
}

impl ProviderClient {
    /// Create a client for the given model, reading `DEEPSEEK_API_KEY` and
    /// `DEEPSEEK_BASE_URL` from the environment.
    pub fn from_model(model: &str) -> Result<Self, ApiError> {
        let _resolved_model = providers::resolve_model_alias(model);
        match providers::detect_provider_kind(model) {
            ProviderKind::Deepseek => Ok(Self::Deepseek(OpenAiCompatClient::from_env(
                OpenAiCompatConfig::deepseek(),
            )?)),
        }
    }

    #[must_use]
    pub const fn provider_kind(&self) -> ProviderKind {
        ProviderKind::Deepseek
    }

    #[must_use]
    pub fn base_url(&self) -> &str {
        match self {
            Self::Deepseek(client) => client.base_url(),
        }
    }

    pub async fn send_message(
        &self,
        request: &MessageRequest,
    ) -> Result<MessageResponse, ApiError> {
        match self {
            Self::Deepseek(client) => client.send_message(request).await,
        }
    }

    pub async fn stream_message(
        &self,
        request: &MessageRequest,
    ) -> Result<MessageStream, ApiError> {
        match self {
            Self::Deepseek(client) => client
                .stream_message(request)
                .await
                .map(MessageStream::OpenAiCompat),
        }
    }

    /// Prompt cache is Anthropic-only. Always returns `None` for DeepSeek.
    #[must_use]
    pub fn take_last_prompt_cache_record(&self) -> Option<crate::prompt_cache::PromptCacheRecord> {
        None
    }
}

#[derive(Debug)]
pub enum MessageStream {
    OpenAiCompat(openai_compat::MessageStream),
}

impl MessageStream {
    #[must_use]
    pub fn request_id(&self) -> Option<&str> {
        match self {
            Self::OpenAiCompat(stream) => stream.request_id(),
        }
    }

    pub async fn next_event(&mut self) -> Result<Option<StreamEvent>, ApiError> {
        match self {
            Self::OpenAiCompat(stream) => stream.next_event().await,
        }
    }
}

/// Read the configured DeepSeek base URL.
#[must_use]
pub fn read_base_url() -> String {
    openai_compat::read_base_url(OpenAiCompatConfig::deepseek())
}

#[cfg(test)]
mod tests {
    use super::ProviderClient;
    use crate::providers::{resolve_model_alias, ProviderKind};

    #[test]
    fn resolves_deepseek_aliases() {
        assert_eq!(resolve_model_alias("pro"), "deepseek-v4-pro");
        assert_eq!(resolve_model_alias("flash"), "deepseek-v4-flash");
        assert_eq!(
            resolve_model_alias("deepseek-v4-pro"),
            "deepseek-v4-pro"
        );
    }

    #[test]
    fn detect_provider_is_always_deepseek() {
        use crate::providers::detect_provider_kind;
        assert_eq!(
            detect_provider_kind("deepseek-v4-pro"),
            ProviderKind::Deepseek
        );
        assert_eq!(
            detect_provider_kind("unknown-model"),
            ProviderKind::Deepseek // fallback
        );
    }
}

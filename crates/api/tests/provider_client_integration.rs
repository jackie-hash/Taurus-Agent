use std::ffi::OsString;
use std::sync::{Mutex, OnceLock};

use api::{ApiError, ProviderClient, ProviderKind};

#[test]
fn provider_client_routes_pro_alias_to_deepseek() {
    let _lock = env_lock();
    let _key = EnvVarGuard::set("DEEPSEEK_API_KEY", Some("test-deepseek-key"));

    let client = ProviderClient::from_model("pro").expect("pro alias should resolve");

    assert_eq!(client.provider_kind(), ProviderKind::Deepseek);
}

#[test]
fn provider_client_routes_flash_alias_to_deepseek() {
    let _lock = env_lock();
    let _key = EnvVarGuard::set("DEEPSEEK_API_KEY", Some("test-deepseek-key"));

    let client = ProviderClient::from_model("flash").expect("flash alias should resolve");

    assert_eq!(client.provider_kind(), ProviderKind::Deepseek);
}

#[test]
fn provider_client_resolves_full_model_name() {
    let _lock = env_lock();
    let _key = EnvVarGuard::set("DEEPSEEK_API_KEY", Some("test-deepseek-key"));

    let client =
        ProviderClient::from_model("deepseek-v4-pro").expect("full model name should resolve");

    assert_eq!(client.provider_kind(), ProviderKind::Deepseek);
}

#[test]
fn provider_client_reports_missing_credentials() {
    let _lock = env_lock();
    let _key = EnvVarGuard::set("DEEPSEEK_API_KEY", None);

    let error = ProviderClient::from_model("deepseek-v4-pro")
        .expect_err("requests without DEEPSEEK_API_KEY should fail");

    match error {
        ApiError::MissingCredentials {
            provider, env_vars, ..
        } => {
            assert_eq!(provider, "DeepSeek");
            assert!(env_vars.contains(&"DEEPSEEK_API_KEY"));
        }
        other => panic!("expected missing credentials, got {other:?}"),
    }
}

fn env_lock() -> std::sync::MutexGuard<'static, ()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

struct EnvVarGuard {
    key: &'static str,
    original: Option<OsString>,
}

impl EnvVarGuard {
    fn set(key: &'static str, value: Option<&str>) -> Self {
        let original = std::env::var_os(key);
        match value {
            Some(value) => std::env::set_var(key, value),
            None => std::env::remove_var(key),
        }
        Self { key, original }
    }
}

impl Drop for EnvVarGuard {
    fn drop(&mut self) {
        match &self.original {
            Some(value) => std::env::set_var(self.key, value),
            None => std::env::remove_var(self.key),
        }
    }
}

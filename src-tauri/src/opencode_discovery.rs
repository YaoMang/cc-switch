use crate::config::get_home_dir;
use crate::error::AppError;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashSet;
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderKeyInfo {
    pub key: String,
    pub has_config: bool,
    pub has_auth: bool,
    pub is_internal: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderKeyDiscoveryResult {
    pub keys: Vec<ProviderKeyInfo>,
    pub config_keys: Vec<String>,
    pub auth_keys: Vec<String>,
    pub internal_keys: Vec<String>,
}

pub fn get_opencode_cache_dir() -> PathBuf {
    get_home_dir().join(".cache").join("opencode")
}

pub fn get_opencode_models_cache_path() -> PathBuf {
    get_opencode_cache_dir().join("models.json")
}

pub fn read_opencode_models_cache_provider_keys() -> HashSet<String> {
    let path = get_opencode_models_cache_path();

    if !path.exists() {
        return HashSet::new();
    }

    let content = match std::fs::read_to_string(&path) {
        Ok(c) => c,
        Err(e) => {
            log::warn!(
                "Failed to read OpenCode models cache ({}): {e}",
                path.display()
            );
            return HashSet::new();
        }
    };

    let parsed: Value = match serde_json::from_str(&content) {
        Ok(v) => v,
        Err(e) => {
            log::warn!(
                "Failed to parse OpenCode models cache ({}): {e}",
                path.display()
            );
            return HashSet::new();
        }
    };

    match parsed {
        Value::Object(map) => map.keys().cloned().collect(),
        _ => {
            log::warn!(
                "OpenCode models cache must be a JSON object, got non-object at {}",
                path.display()
            );
            HashSet::new()
        }
    }
}

pub fn discover_opencode_provider_keys() -> Result<ProviderKeyDiscoveryResult, AppError> {
    let config_keys: HashSet<String> = crate::opencode_config::get_providers()?
        .keys()
        .cloned()
        .collect();

    let auth_keys: HashSet<String> = crate::opencode_auth::read_opencode_auth()?
        .keys()
        .cloned()
        .collect();

    let internal_keys: HashSet<String> = read_opencode_models_cache_provider_keys();

    let all_keys: Vec<String> = {
        let mut combined: Vec<String> = config_keys.union(&auth_keys).cloned().collect();
        combined.sort();
        combined
    };

    let keys: Vec<ProviderKeyInfo> = all_keys
        .into_iter()
        .map(|key| {
            let has_config = config_keys.contains(&key);
            let has_auth = auth_keys.contains(&key);
            let is_internal = internal_keys.contains(&key);
            ProviderKeyInfo {
                key,
                has_config,
                has_auth,
                is_internal,
            }
        })
        .collect();

    let mut config_keys_sorted: Vec<String> = config_keys.into_iter().collect();
    config_keys_sorted.sort();
    let mut auth_keys_sorted: Vec<String> = auth_keys.into_iter().collect();
    auth_keys_sorted.sort();
    let mut internal_keys_sorted: Vec<String> = internal_keys.into_iter().collect();
    internal_keys_sorted.sort();

    Ok(ProviderKeyDiscoveryResult {
        keys,
        config_keys: config_keys_sorted,
        auth_keys: auth_keys_sorted,
        internal_keys: internal_keys_sorted,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serial_test::serial;
    use std::fs;

    struct TempHome {
        dir: PathBuf,
        old_var: Option<std::ffi::OsString>,
    }

    impl TempHome {
        fn new() -> Self {
            let dir = std::env::temp_dir().join(format!(
                "cc-switch-discovery-test-{}",
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_nanos()
            ));
            let _ = fs::create_dir_all(&dir);
            let home = dir.join("home");
            let _ = fs::create_dir_all(home.join(".cache").join("opencode"));
            let _ = fs::create_dir_all(home.join(".local").join("share").join("opencode"));
            let _ = fs::create_dir_all(home.join(".config").join("opencode"));

            let old_var = std::env::var_os("CC_SWITCH_TEST_HOME");
            std::env::set_var("CC_SWITCH_TEST_HOME", home.to_str().unwrap());

            Self { dir, old_var }
        }

        fn models_cache_path(&self) -> PathBuf {
            self.dir
                .join("home")
                .join(".cache")
                .join("opencode")
                .join("models.json")
        }

        fn auth_path(&self) -> PathBuf {
            self.dir
                .join("home")
                .join(".local")
                .join("share")
                .join("opencode")
                .join("auth.json")
        }

        fn config_path(&self) -> PathBuf {
            self.dir
                .join("home")
                .join(".config")
                .join("opencode")
                .join("opencode.json")
        }
    }

    impl Drop for TempHome {
        fn drop(&mut self) {
            match &self.old_var {
                Some(v) => std::env::set_var("CC_SWITCH_TEST_HOME", v),
                None => std::env::remove_var("CC_SWITCH_TEST_HOME"),
            }
            let _ = fs::remove_dir_all(&self.dir);
        }
    }

    #[test]
    #[serial]
    fn missing_models_cache_returns_empty_set() {
        let _th = TempHome::new();
        let keys = read_opencode_models_cache_provider_keys();
        assert!(keys.is_empty());
    }

    #[test]
    #[serial]
    fn missing_models_cache_does_not_create_dir() {
        let th = TempHome::new();
        let cache_dir = th.dir.join("home").join(".cache").join("opencode");
        let _ = fs::remove_dir_all(&cache_dir);
        assert!(!cache_dir.exists());

        let keys = read_opencode_models_cache_provider_keys();
        assert!(keys.is_empty());
        assert!(
            !cache_dir.exists(),
            "read_opencode_models_cache_provider_keys must not create the cache directory"
        );
    }

    #[test]
    #[serial]
    fn invalid_json_models_cache_returns_empty_set() {
        let th = TempHome::new();
        fs::write(th.models_cache_path(), "not valid json").unwrap();

        let keys = read_opencode_models_cache_provider_keys();
        assert!(keys.is_empty());
    }

    #[test]
    #[serial]
    fn non_object_models_cache_returns_empty_set() {
        let th = TempHome::new();
        fs::write(th.models_cache_path(), r#"[1, 2, 3]"#).unwrap();

        let keys = read_opencode_models_cache_provider_keys();
        assert!(keys.is_empty());
    }

    #[test]
    #[serial]
    fn valid_models_cache_returns_provider_keys() {
        let th = TempHome::new();
        fs::write(
            th.models_cache_path(),
            r#"{"anthropic": {"id": "anthropic"}, "openai": {"id": "openai"}, "google": {"id": "google"}}"#,
        )
        .unwrap();

        let keys = read_opencode_models_cache_provider_keys();
        let expected: HashSet<String> = ["anthropic", "openai", "google"]
            .into_iter()
            .map(String::from)
            .collect();

        assert_eq!(keys, expected);
    }

    #[test]
    #[serial]
    fn discover_with_all_sources_empty() {
        let _th = TempHome::new();
        let result = discover_opencode_provider_keys().unwrap();
        assert!(result.keys.is_empty());
        assert!(result.config_keys.is_empty());
        assert!(result.auth_keys.is_empty());
        assert!(result.internal_keys.is_empty());
    }

    #[test]
    #[serial]
    fn discover_with_config_only() {
        let th = TempHome::new();
        fs::write(
            th.config_path(),
            r#"{
                "$schema": "https://opencode.ai/config.json",
                "provider": {
                    "my-custom-provider": {"npm": "@ai-sdk/custom"}
                }
            }"#,
        )
        .unwrap();

        let result = discover_opencode_provider_keys().unwrap();
        assert_eq!(result.keys.len(), 1);
        assert_eq!(result.keys[0].key, "my-custom-provider");
        assert!(result.keys[0].has_config);
        assert!(!result.keys[0].has_auth);
        assert!(!result.keys[0].is_internal);
        assert_eq!(result.config_keys.len(), 1);
        assert!(result.auth_keys.is_empty());
        assert!(result.internal_keys.is_empty());
    }

    #[test]
    #[serial]
    fn discover_with_auth_only() {
        let th = TempHome::new();
        fs::write(
            th.auth_path(),
            r#"{"auth-only-provider": {"type": "api", "key": "FAKE_KEY"}}"#,
        )
        .unwrap();

        let result = discover_opencode_provider_keys().unwrap();
        assert_eq!(result.keys.len(), 1);
        assert_eq!(result.keys[0].key, "auth-only-provider");
        assert!(!result.keys[0].has_config);
        assert!(result.keys[0].has_auth);
        assert!(!result.keys[0].is_internal);
        assert!(result.config_keys.is_empty());
        assert_eq!(result.auth_keys.len(), 1);
        assert!(result.internal_keys.is_empty());
    }

    #[test]
    #[serial]
    fn discover_with_internal_only() {
        let th = TempHome::new();
        fs::write(
            th.models_cache_path(),
            r#"{"anthropic": {"id": "anthropic"}, "openai": {"id": "openai"}}"#,
        )
        .unwrap();

        let result = discover_opencode_provider_keys().unwrap();
        assert!(
            result.keys.is_empty(),
            "internal-only keys should not appear as discovered keys"
        );
        assert!(result.config_keys.is_empty());
        assert!(result.auth_keys.is_empty());
        assert_eq!(result.internal_keys.len(), 2);
        assert!(result.internal_keys.contains(&"anthropic".to_string()));
        assert!(result.internal_keys.contains(&"openai".to_string()));
    }

    #[test]
    #[serial]
    fn discover_combines_all_sources() {
        let th = TempHome::new();
        fs::write(
            th.config_path(),
            r#"{
                "$schema": "https://opencode.ai/config.json",
                "provider": {
                    "config-only": {"npm": "@ai-sdk/custom"},
                    "in-both": {"npm": "@ai-sdk/both"}
                }
            }"#,
        )
        .unwrap();
        fs::write(
            th.auth_path(),
            r#"{"auth-only": {"type": "api", "key": "FAKE_KEY"}, "in-both": {"type": "api", "key": "FAKE_KEY"}}"#,
        )
        .unwrap();
        fs::write(
            th.models_cache_path(),
            r#"{"anthropic": {"id": "anthropic"}, "in-both": {"id": "in-both"}}"#,
        )
        .unwrap();

        let result = discover_opencode_provider_keys().unwrap();
        assert_eq!(result.keys.len(), 3);

        let auth_only = result.keys.iter().find(|k| k.key == "auth-only").unwrap();
        assert!(!auth_only.has_config);
        assert!(auth_only.has_auth);
        assert!(!auth_only.is_internal);

        let config_only = result.keys.iter().find(|k| k.key == "config-only").unwrap();
        assert!(config_only.has_config);
        assert!(!config_only.has_auth);
        assert!(!config_only.is_internal);

        let in_both = result.keys.iter().find(|k| k.key == "in-both").unwrap();
        assert!(in_both.has_config);
        assert!(in_both.has_auth);
        assert!(in_both.is_internal);
    }

    #[test]
    #[serial]
    fn discover_missing_auth_file_is_non_fatal() {
        let th = TempHome::new();
        fs::write(
            th.config_path(),
            r#"{
                "$schema": "https://opencode.ai/config.json",
                "provider": {
                    "config-only": {"npm": "@ai-sdk/custom"}
                }
            }"#,
        )
        .unwrap();

        let result = discover_opencode_provider_keys().unwrap();
        assert_eq!(result.keys.len(), 1);
        assert_eq!(result.keys[0].key, "config-only");
        assert!(result.keys[0].has_config);
        assert!(!result.keys[0].has_auth);
    }

    #[test]
    #[serial]
    fn discover_invalid_auth_returns_error() {
        let th = TempHome::new();
        fs::write(th.auth_path(), "{invalid}").unwrap();

        let result = discover_opencode_provider_keys();
        assert!(
            result.is_err(),
            "invalid auth.json should propagate error per stage1 safety rules"
        );
    }

    #[test]
    #[serial]
    fn discover_missing_models_cache_is_non_fatal() {
        let th = TempHome::new();
        fs::write(
            th.config_path(),
            r#"{
                "$schema": "https://opencode.ai/config.json",
                "provider": {
                    "my-provider": {"npm": "@ai-sdk/custom"}
                }
            }"#,
        )
        .unwrap();
        fs::write(
            th.auth_path(),
            r#"{"my-provider": {"type": "api", "key": "FAKE_KEY"}}"#,
        )
        .unwrap();

        let result = discover_opencode_provider_keys().unwrap();
        assert_eq!(result.keys.len(), 1);
        assert!(result.keys[0].has_config);
        assert!(result.keys[0].has_auth);
        assert!(!result.keys[0].is_internal);
        assert!(result.internal_keys.is_empty());
    }

    #[test]
    #[serial]
    fn discover_keys_are_sorted() {
        let th = TempHome::new();
        fs::write(
            th.config_path(),
            r#"{
                "$schema": "https://opencode.ai/config.json",
                "provider": {
                    "z-provider": {"npm": "@ai-sdk/z"},
                    "a-provider": {"npm": "@ai-sdk/a"},
                    "m-provider": {"npm": "@ai-sdk/m"}
                }
            }"#,
        )
        .unwrap();

        let result = discover_opencode_provider_keys().unwrap();
        let keys: Vec<&str> = result.keys.iter().map(|k| k.key.as_str()).collect();
        assert_eq!(keys, vec!["a-provider", "m-provider", "z-provider"]);
    }

    #[test]
    #[serial]
    fn get_opencode_cache_dir_returns_cache_path() {
        let _th = TempHome::new();
        let path = get_opencode_cache_dir();
        assert!(path.to_string_lossy().contains(".cache"));
        assert!(path.to_string_lossy().contains("opencode"));
    }

    #[test]
    #[serial]
    fn get_opencode_models_cache_path_ends_with_models_json() {
        let _th = TempHome::new();
        let path = get_opencode_models_cache_path();
        assert_eq!(path.file_name().unwrap(), "models.json");
        assert!(path.to_string_lossy().contains(".cache"));
    }

    #[test]
    #[serial]
    fn discover_internal_partial_override_without_npm() {
        let th = TempHome::new();
        fs::write(
            th.config_path(),
            r#"{
                "$schema": "https://opencode.ai/config.json",
                "provider": {
                    "anthropic": {
                        "options": {
                            "baseURL": "https://example.com"
                        }
                    }
                }
            }"#,
        )
        .unwrap();
        fs::write(
            th.models_cache_path(),
            r#"{"anthropic": {"id": "anthropic"}}"#,
        )
        .unwrap();

        let result = discover_opencode_provider_keys().unwrap();
        assert_eq!(result.keys.len(), 1);
        assert_eq!(result.keys[0].key, "anthropic");
        assert!(result.keys[0].has_config);
        assert!(!result.keys[0].has_auth);
        assert!(result.keys[0].is_internal);
        assert_eq!(result.config_keys.len(), 1);
        assert!(result.auth_keys.is_empty());
        assert!(result.internal_keys.contains(&"anthropic".to_string()));
    }

    #[test]
    #[serial]
    fn discover_non_internal_partial_config_remains_non_internal() {
        let th = TempHome::new();
        fs::write(
            th.config_path(),
            r#"{
                "$schema": "https://opencode.ai/config.json",
                "provider": {
                    "some-custom": {
                        "options": {
                            "baseURL": "https://example.com"
                        }
                    }
                }
            }"#,
        )
        .unwrap();

        let result = discover_opencode_provider_keys().unwrap();
        assert_eq!(result.keys.len(), 1);
        assert_eq!(result.keys[0].key, "some-custom");
        assert!(result.keys[0].has_config);
        assert!(!result.keys[0].has_auth);
        assert!(!result.keys[0].is_internal);
        assert_eq!(result.config_keys.len(), 1);
        assert!(result.auth_keys.is_empty());
        assert!(result.internal_keys.is_empty());
    }
}

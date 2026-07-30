pub mod config;
pub mod data_layout;

use std::ffi::OsString;
use std::fmt;
use std::path::PathBuf;

use data_layout::CODEX_HOME_ENV;

pub const DEFAULT_BASE_URL: &str = "https://miao.313619.xyz";
pub const DEFAULT_API_KEY_ENV: &str = "DOUSTACK_API_KEY";
pub const DEFAULT_CODEX_BINARY: &str = "codex";
pub const PROVIDER_ID: &str = "doustack";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderSettings {
    pub base_url: String,
    pub api_key_env: String,
    pub model: Option<String>,
}

impl Default for ProviderSettings {
    fn default() -> Self {
        Self {
            base_url: DEFAULT_BASE_URL.to_string(),
            api_key_env: DEFAULT_API_KEY_ENV.to_string(),
            model: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LaunchRequest {
    pub codex_binary: String,
    pub codex_home: PathBuf,
    pub forwarded_args: Vec<String>,
}

impl Default for LaunchRequest {
    fn default() -> Self {
        Self {
            codex_binary: DEFAULT_CODEX_BINARY.to_string(),
            codex_home: PathBuf::from(".dscode/codex"),
            forwarded_args: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LaunchPlan {
    pub program: String,
    pub args: Vec<String>,
    pub environment: Vec<(OsString, OsString)>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConfigError {
    EmptyBaseUrl,
    InsecureBaseUrl,
    BaseUrlContainsQueryOrFragment,
    InvalidApiKeyEnvironment,
    EmptyModel,
    EmptyCodexBinary,
    EmptyCodexHome,
}

impl fmt::Display for ConfigError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyBaseUrl => f.write_str("DouStack base URL cannot be empty"),
            Self::InsecureBaseUrl => f.write_str("DouStack base URL must use HTTPS"),
            Self::BaseUrlContainsQueryOrFragment => {
                f.write_str("DouStack base URL cannot contain a query or fragment")
            }
            Self::InvalidApiKeyEnvironment => {
                f.write_str("API-key environment variable name is invalid")
            }
            Self::EmptyModel => f.write_str("model cannot be empty"),
            Self::EmptyCodexBinary => f.write_str("Codex binary cannot be empty"),
            Self::EmptyCodexHome => f.write_str("Codex home cannot be empty"),
        }
    }
}

impl std::error::Error for ConfigError {}

/// The stable seam between DS Code clients and Codex provider configuration.
pub struct CodexRuntime;

impl CodexRuntime {
    pub fn plan(
        settings: &ProviderSettings,
        request: LaunchRequest,
    ) -> Result<LaunchPlan, ConfigError> {
        validate(settings, &request)?;

        let mut args = Vec::with_capacity(14 + request.forwarded_args.len());
        push_override(&mut args, "model_provider", PROVIDER_ID);
        push_override(&mut args, "model_providers.doustack.name", "OpenAI");
        push_override(
            &mut args,
            "model_providers.doustack.base_url",
            settings.base_url.trim_end_matches('/'),
        );
        push_override(
            &mut args,
            "model_providers.doustack.env_key",
            &settings.api_key_env,
        );
        push_override(&mut args, "model_providers.doustack.wire_api", "responses");
        args.push("--config".to_string());
        args.push("model_providers.doustack.requires_openai_auth=false".to_string());
        args.push("--config".to_string());
        args.push("model_providers.doustack.supports_websockets=false".to_string());

        if let Some(model) = settings.model.as_deref() {
            args.push("--model".to_string());
            args.push(model.to_string());
        }

        args.extend(request.forwarded_args);

        Ok(LaunchPlan {
            program: request.codex_binary,
            args,
            environment: vec![(
                OsString::from(CODEX_HOME_ENV),
                request.codex_home.into_os_string(),
            )],
        })
    }

    pub fn render_config(settings: &ProviderSettings) -> Result<String, ConfigError> {
        validate(settings, &LaunchRequest::default())?;

        let mut output = String::new();
        if let Some(model) = settings.model.as_deref() {
            output.push_str("model = ");
            output.push_str(&toml_string(model));
            output.push('\n');
        }
        output.push_str("model_provider = \"doustack\"\n\n");
        output.push_str("[model_providers.doustack]\n");
        output.push_str("name = \"OpenAI\"\n");
        output.push_str("base_url = ");
        output.push_str(&toml_string(settings.base_url.trim_end_matches('/')));
        output.push('\n');
        output.push_str("env_key = ");
        output.push_str(&toml_string(&settings.api_key_env));
        output.push('\n');
        output.push_str("wire_api = \"responses\"\n");
        output.push_str("requires_openai_auth = false\n");
        output.push_str("supports_websockets = false\n");
        Ok(output)
    }
}

fn validate(settings: &ProviderSettings, request: &LaunchRequest) -> Result<(), ConfigError> {
    let base_url = settings.base_url.trim();
    if base_url.is_empty() {
        return Err(ConfigError::EmptyBaseUrl);
    }
    if !base_url.starts_with("https://") {
        return Err(ConfigError::InsecureBaseUrl);
    }
    if base_url.contains('?') || base_url.contains('#') {
        return Err(ConfigError::BaseUrlContainsQueryOrFragment);
    }
    if !is_environment_name(&settings.api_key_env) {
        return Err(ConfigError::InvalidApiKeyEnvironment);
    }
    if settings
        .model
        .as_ref()
        .is_some_and(|model| model.trim().is_empty())
    {
        return Err(ConfigError::EmptyModel);
    }
    if request.codex_binary.trim().is_empty() {
        return Err(ConfigError::EmptyCodexBinary);
    }
    if request.codex_home.as_os_str().is_empty() {
        return Err(ConfigError::EmptyCodexHome);
    }
    Ok(())
}

fn is_environment_name(value: &str) -> bool {
    let mut chars = value.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    (first == '_' || first.is_ascii_uppercase())
        && chars.all(|character| {
            character == '_' || character.is_ascii_uppercase() || character.is_ascii_digit()
        })
}

fn push_override(args: &mut Vec<String>, key: &str, value: &str) {
    args.push("--config".to_string());
    args.push(format!("{key}={}", toml_string(value)));
}

fn toml_string(value: &str) -> String {
    let mut output = String::with_capacity(value.len() + 2);
    output.push('"');
    for character in value.chars() {
        match character {
            '\\' => output.push_str("\\\\"),
            '"' => output.push_str("\\\""),
            '\n' => output.push_str("\\n"),
            '\r' => output.push_str("\\r"),
            '\t' => output.push_str("\\t"),
            other => output.push(other),
        }
    }
    output.push('"');
    output
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn launch_plan_injects_doustack_without_a_secret() {
        let settings = ProviderSettings {
            model: Some("gpt-example".to_string()),
            ..ProviderSettings::default()
        };

        let plan = CodexRuntime::plan(
            &settings,
            LaunchRequest {
                forwarded_args: vec!["exec".to_string(), "inspect this repo".to_string()],
                ..LaunchRequest::default()
            },
        )
        .expect("valid launch plan");

        assert_eq!(plan.program, "codex");
        assert!(
            plan.args
                .contains(&"model_provider=\"doustack\"".to_string())
        );
        assert!(plan.args.contains(&"--model".to_string()));
        assert!(plan.args.contains(&"gpt-example".to_string()));
        assert!(
            plan.args
                .ends_with(&["exec".to_string(), "inspect this repo".to_string()])
        );
        assert!(!plan.args.join(" ").contains("API_KEY="));
        assert_eq!(
            plan.environment,
            vec![(
                OsString::from("CODEX_HOME"),
                OsString::from(".dscode/codex")
            )]
        );
    }

    #[test]
    fn rendered_config_references_the_environment_without_a_secret() {
        let rendered = CodexRuntime::render_config(&ProviderSettings::default())
            .expect("valid provider config");

        assert!(rendered.contains("env_key = \"DOUSTACK_API_KEY\""));
        assert!(rendered.contains("name = \"OpenAI\""));
        assert!(rendered.contains("requires_openai_auth = false"));
        assert!(!rendered.contains("bearer_token"));
    }

    #[test]
    fn invalid_provider_settings_are_rejected() {
        let settings = ProviderSettings {
            base_url: "http://example.test/v1".to_string(),
            ..ProviderSettings::default()
        };

        assert_eq!(
            CodexRuntime::plan(&settings, LaunchRequest::default()),
            Err(ConfigError::InsecureBaseUrl)
        );
    }

    #[test]
    fn trailing_slash_is_removed_from_the_api_root() {
        let settings = ProviderSettings {
            base_url: "https://example.test/v1/".to_string(),
            ..ProviderSettings::default()
        };

        let plan =
            CodexRuntime::plan(&settings, LaunchRequest::default()).expect("valid launch plan");

        assert!(plan.args.contains(
            &"model_providers.doustack.base_url=\"https://example.test/v1\"".to_string()
        ));
    }
}

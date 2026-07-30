use std::fmt;
use std::fs::{self, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::{CodexRuntime, DEFAULT_API_KEY_ENV, DEFAULT_BASE_URL, ProviderSettings};

pub const CONFIG_VERSION: u32 = 1;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct AppConfig {
    pub version: u32,
    pub provider: ProviderConfig,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            version: CONFIG_VERSION,
            provider: ProviderConfig::default(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct ProviderConfig {
    pub base_url: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
}

impl Default for ProviderConfig {
    fn default() -> Self {
        Self {
            base_url: DEFAULT_BASE_URL.to_string(),
            model: None,
        }
    }
}

#[derive(Debug)]
pub enum AppConfigError {
    Read {
        path: PathBuf,
        source: io::Error,
    },
    Parse {
        path: PathBuf,
        source: toml::de::Error,
    },
    UnsupportedVersion(u32),
    Invalid(String),
    Encode(toml::ser::Error),
    Write {
        path: PathBuf,
        source: io::Error,
    },
}

impl fmt::Display for AppConfigError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Read { path, source } => {
                write!(f, "cannot read {}: {source}", path.display())
            }
            Self::Parse { path, source } => {
                write!(f, "cannot parse {}: {source}", path.display())
            }
            Self::UnsupportedVersion(version) => {
                write!(f, "unsupported DS Code config version {version}")
            }
            Self::Invalid(message) => write!(f, "invalid DS Code config: {message}"),
            Self::Encode(source) => write!(f, "cannot encode DS Code config: {source}"),
            Self::Write { path, source } => {
                write!(f, "cannot write {}: {source}", path.display())
            }
        }
    }
}

impl std::error::Error for AppConfigError {}

impl AppConfig {
    pub fn load(path: &Path) -> Result<Self, AppConfigError> {
        let contents = match fs::read_to_string(path) {
            Ok(contents) => contents,
            Err(source) if source.kind() == io::ErrorKind::NotFound => return Ok(Self::default()),
            Err(source) => {
                return Err(AppConfigError::Read {
                    path: path.to_path_buf(),
                    source,
                });
            }
        };
        let config: Self = toml::from_str(&contents).map_err(|source| AppConfigError::Parse {
            path: path.to_path_buf(),
            source,
        })?;
        config.validate()?;
        Ok(config)
    }

    pub fn save(&self, path: &Path) -> Result<(), AppConfigError> {
        self.validate()?;
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|source| AppConfigError::Write {
                path: parent.to_path_buf(),
                source,
            })?;
        }

        let contents = toml::to_string_pretty(self).map_err(AppConfigError::Encode)?;
        let mut options = OpenOptions::new();
        options.create(true).truncate(true).write(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let mut file = options.open(path).map_err(|source| AppConfigError::Write {
            path: path.to_path_buf(),
            source,
        })?;
        file.write_all(contents.as_bytes())
            .and_then(|()| file.sync_all())
            .map_err(|source| AppConfigError::Write {
                path: path.to_path_buf(),
                source,
            })
    }

    pub fn provider_settings(&self) -> ProviderSettings {
        ProviderSettings {
            base_url: self.provider.base_url.clone(),
            api_key_env: DEFAULT_API_KEY_ENV.to_string(),
            model: self.provider.model.clone(),
        }
    }

    fn validate(&self) -> Result<(), AppConfigError> {
        if self.version != CONFIG_VERSION {
            return Err(AppConfigError::UnsupportedVersion(self.version));
        }
        CodexRuntime::render_config(&self.provider_settings())
            .map(|_| ())
            .map_err(|error| AppConfigError::Invalid(error.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn test_path(name: &str) -> PathBuf {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock")
            .as_nanos();
        std::env::temp_dir().join(format!(
            "dscode-config-{name}-{}-{unique}/config.toml",
            std::process::id()
        ))
    }

    #[test]
    fn missing_config_uses_defaults() {
        let path = test_path("missing");
        let config = AppConfig::load(&path).expect("default config");

        assert_eq!(config, AppConfig::default());
    }

    #[test]
    fn config_round_trip_contains_no_credential() {
        let path = test_path("round-trip");
        let mut config = AppConfig::default();
        config.provider.model = Some("gpt-example".to_string());

        config.save(&path).expect("save config");
        let contents = fs::read_to_string(&path).expect("read persisted config");
        let loaded = AppConfig::load(&path).expect("load persisted config");

        assert_eq!(loaded, config);
        assert!(!contents.contains("api_key"));
        assert!(!contents.contains("credential"));

        fs::remove_dir_all(path.parent().expect("config parent"))
            .expect("remove isolated test directory");
    }

    #[test]
    fn unsupported_version_is_rejected() {
        let path = test_path("version");
        fs::create_dir_all(path.parent().expect("config parent")).expect("create test directory");
        fs::write(&path, "version = 99\n").expect("write test config");

        let error = AppConfig::load(&path).expect_err("reject unknown version");

        assert!(matches!(error, AppConfigError::UnsupportedVersion(99)));
        fs::remove_dir_all(path.parent().expect("config parent"))
            .expect("remove isolated test directory");
    }
}

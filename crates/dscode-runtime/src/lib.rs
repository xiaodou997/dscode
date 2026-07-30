mod app_server;
mod contract;

pub use app_server::AppServerClient;
pub use contract::{ProviderCapabilities, ReadOnlyContractReport, run_read_only_contract};

use std::ffi::OsString;
use std::fmt;
use std::io;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitStatus};
use std::time::Duration;

use semver::Version;

pub const TESTED_CODEX_VERSION: &str = "0.146.0";
pub const DEFAULT_PROBE_TIMEOUT: Duration = Duration::from_secs(5);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Compatibility {
    Tested,
    OlderUntested,
    NewerUntested,
}

impl Compatibility {
    pub fn is_tested(self) -> bool {
        self == Self::Tested
    }

    pub fn description(self) -> &'static str {
        match self {
            Self::Tested => "tested",
            Self::OlderUntested => "older than the tested runtime",
            Self::NewerUntested => "newer than the tested runtime",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeInspection {
    pub version: Version,
    pub compatibility: Compatibility,
    pub raw_version: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppServerInfo {
    pub user_agent: String,
    pub codex_home: PathBuf,
    pub platform_family: String,
    pub platform_os: String,
}

#[derive(Debug)]
pub enum RuntimeError {
    Launch {
        program: String,
        source: io::Error,
    },
    CommandFailed {
        program: String,
        status: ExitStatus,
        stderr: String,
    },
    InvalidVersion {
        output: String,
    },
    Io {
        operation: &'static str,
        source: io::Error,
    },
    ProbeTimeout(Duration),
    RequestTimeout {
        method: String,
        timeout: Duration,
    },
    Disconnected {
        stderr: String,
    },
    InvalidProtocol(String),
    Protocol {
        code: Option<i64>,
        message: String,
    },
}

impl fmt::Display for RuntimeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Launch { program, source } => write!(f, "cannot start `{program}`: {source}"),
            Self::CommandFailed {
                program,
                status,
                stderr,
            } => {
                write!(f, "`{program}` exited with {status}")?;
                if !stderr.trim().is_empty() {
                    write!(f, ": {}", stderr.trim())?;
                }
                Ok(())
            }
            Self::InvalidVersion { output } => {
                write!(f, "cannot parse Codex version from `{}`", output.trim())
            }
            Self::Io { operation, source } => write!(f, "app-server {operation} failed: {source}"),
            Self::ProbeTimeout(timeout) => {
                write!(f, "app-server initialization timed out after {timeout:?}")
            }
            Self::RequestTimeout { method, timeout } => {
                write!(f, "app-server `{method}` timed out after {timeout:?}")
            }
            Self::Disconnected { stderr } => {
                f.write_str("app-server disconnected")?;
                if !stderr.is_empty() {
                    write!(f, ": {stderr}")?;
                }
                Ok(())
            }
            Self::InvalidProtocol(message) => write!(f, "invalid app-server response: {message}"),
            Self::Protocol { code, message } => match code {
                Some(code) => write!(f, "app-server error {code}: {message}"),
                None => write!(f, "app-server error: {message}"),
            },
        }
    }
}

impl std::error::Error for RuntimeError {}

pub fn inspect_runtime(program: &str) -> Result<RuntimeInspection, RuntimeError> {
    let output = Command::new(program)
        .arg("--version")
        .output()
        .map_err(|source| RuntimeError::Launch {
            program: program.to_string(),
            source,
        })?;
    if !output.status.success() {
        return Err(RuntimeError::CommandFailed {
            program: program.to_string(),
            status: output.status,
            stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
        });
    }

    let raw_version = String::from_utf8_lossy(&output.stdout).trim().to_string();
    let version = parse_codex_version(&raw_version)?;
    Ok(RuntimeInspection {
        compatibility: compatibility_for(&version),
        version,
        raw_version,
    })
}

pub fn parse_codex_version(output: &str) -> Result<Version, RuntimeError> {
    output
        .split_whitespace()
        .filter_map(|part| Version::parse(part.trim_start_matches('v')).ok())
        .next()
        .ok_or_else(|| RuntimeError::InvalidVersion {
            output: output.to_string(),
        })
}

pub fn compatibility_for(version: &Version) -> Compatibility {
    let tested = Version::parse(TESTED_CODEX_VERSION).expect("tested Codex version is valid");
    match version.cmp(&tested) {
        std::cmp::Ordering::Equal => Compatibility::Tested,
        std::cmp::Ordering::Less => Compatibility::OlderUntested,
        std::cmp::Ordering::Greater => Compatibility::NewerUntested,
    }
}

pub fn probe_app_server(
    program: &str,
    codex_home: &Path,
    client_version: &str,
) -> Result<AppServerInfo, RuntimeError> {
    probe_app_server_with_timeout(program, codex_home, client_version, DEFAULT_PROBE_TIMEOUT)
}

pub fn probe_app_server_with_timeout(
    program: &str,
    codex_home: &Path,
    client_version: &str,
    timeout: Duration,
) -> Result<AppServerInfo, RuntimeError> {
    let args = vec![
        "app-server".to_string(),
        "--listen".to_string(),
        "stdio://".to_string(),
    ];
    let environment = vec![(
        OsString::from("CODEX_HOME"),
        codex_home.as_os_str().to_os_string(),
    )];
    let (client, info) =
        AppServerClient::start(program, &args, &environment, client_version, timeout)?;
    if !same_path(&info.codex_home, codex_home) {
        return Err(RuntimeError::InvalidProtocol(format!(
            "app-server used CODEX_HOME `{}` instead of `{}`",
            info.codex_home.display(),
            codex_home.display()
        )));
    }
    drop(client);
    Ok(info)
}

fn same_path(left: &Path, right: &Path) -> bool {
    let left = left.canonicalize().unwrap_or_else(|_| left.to_path_buf());
    let right = right.canonicalize().unwrap_or_else(|_| right.to_path_buf());
    left == right
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Value;

    #[test]
    fn parses_codex_cli_version_output() {
        let version = parse_codex_version("codex-cli 0.146.0\n").expect("valid version");

        assert_eq!(version, Version::new(0, 146, 0));
        assert_eq!(compatibility_for(&version), Compatibility::Tested);
    }

    #[test]
    fn classifies_unverified_versions() {
        assert_eq!(
            compatibility_for(&Version::new(0, 145, 0)),
            Compatibility::OlderUntested
        );
        assert_eq!(
            compatibility_for(&Version::new(0, 147, 0)),
            Compatibility::NewerUntested
        );
    }

    #[test]
    fn rejects_output_without_a_semantic_version() {
        let error = parse_codex_version("codex unknown").expect_err("invalid version");

        assert!(matches!(error, RuntimeError::InvalidVersion { .. }));
    }

    #[test]
    fn pinned_schema_matches_the_tested_runtime() {
        let manifest = include_str!("../../../schemas/codex/0.146.0/manifest.toml");
        let schema =
            include_str!("../../../schemas/codex/0.146.0/codex_app_server_protocol.schemas.json");
        let parsed: Value = serde_json::from_str(schema).expect("valid pinned JSON schema");

        assert!(manifest.contains(&format!("codex_version = \"{TESTED_CODEX_VERSION}\"")));
        assert_eq!(parsed["title"], "CodexAppServerProtocol");
        assert!(parsed["definitions"].is_object());
    }
}

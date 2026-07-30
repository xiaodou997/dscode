use std::fmt;
use std::io::{self, BufRead, BufReader, Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, ExitStatus, Stdio};
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};

use semver::Version;
use serde::Deserialize;
use serde_json::{Value, json};

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
    let child = Command::new(program)
        .args(["app-server", "--listen", "stdio://"])
        .env("CODEX_HOME", codex_home)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|source| RuntimeError::Launch {
            program: program.to_string(),
            source,
        })?;
    let mut child = ManagedChild(child);

    let mut stdin =
        child.0.stdin.take().ok_or_else(|| {
            RuntimeError::InvalidProtocol("stdin pipe is unavailable".to_string())
        })?;
    let stdout =
        child.0.stdout.take().ok_or_else(|| {
            RuntimeError::InvalidProtocol("stdout pipe is unavailable".to_string())
        })?;
    let stderr =
        child.0.stderr.take().ok_or_else(|| {
            RuntimeError::InvalidProtocol("stderr pipe is unavailable".to_string())
        })?;

    let stderr_reader = thread::spawn(move || {
        let mut output = String::new();
        let _ = BufReader::new(stderr).read_to_string(&mut output);
        output
    });
    let (response_sender, response_receiver) = mpsc::channel();
    let stdout_reader = thread::spawn(move || {
        let mut line = String::new();
        let result = BufReader::new(stdout).read_line(&mut line).map(|_| line);
        let _ = response_sender.send(result);
    });

    let initialize = json!({
        "method": "initialize",
        "id": 1,
        "params": {
            "clientInfo": {
                "name": "dscode",
                "title": "DS Code",
                "version": client_version
            }
        }
    });
    write_json_line(&mut stdin, &initialize)?;

    let response = match response_receiver.recv_timeout(timeout) {
        Ok(Ok(response)) if !response.is_empty() => response,
        Ok(Ok(_)) => {
            stop_child(&mut child.0, stdin);
            let _ = stdout_reader.join();
            let stderr = stderr_reader.join().unwrap_or_default();
            return Err(RuntimeError::InvalidProtocol(if stderr.trim().is_empty() {
                "app-server closed stdout before initialize completed".to_string()
            } else {
                stderr.trim().to_string()
            }));
        }
        Ok(Err(source)) => {
            stop_child(&mut child.0, stdin);
            let _ = stdout_reader.join();
            let _ = stderr_reader.join();
            return Err(RuntimeError::Io {
                operation: "stdout read",
                source,
            });
        }
        Err(mpsc::RecvTimeoutError::Timeout) => {
            stop_child(&mut child.0, stdin);
            let _ = stdout_reader.join();
            let _ = stderr_reader.join();
            return Err(RuntimeError::ProbeTimeout(timeout));
        }
        Err(mpsc::RecvTimeoutError::Disconnected) => {
            stop_child(&mut child.0, stdin);
            let _ = stdout_reader.join();
            let _ = stderr_reader.join();
            return Err(RuntimeError::InvalidProtocol(
                "app-server response reader stopped unexpectedly".to_string(),
            ));
        }
    };

    let parsed = parse_initialize_response(&response);
    let initialized_write = if parsed.is_ok() {
        let initialized = json!({ "method": "initialized" });
        write_json_line(&mut stdin, &initialized)
    } else {
        Ok(())
    };
    drop(stdin);
    finish_child(&mut child.0);
    let _ = stdout_reader.join();
    let _ = stderr_reader.join();
    initialized_write?;
    let info = parsed?;
    if !same_path(&info.codex_home, codex_home) {
        return Err(RuntimeError::InvalidProtocol(format!(
            "app-server used CODEX_HOME `{}` instead of `{}`",
            info.codex_home.display(),
            codex_home.display()
        )));
    }
    Ok(info)
}

fn write_json_line(stdin: &mut impl Write, value: &Value) -> Result<(), RuntimeError> {
    serde_json::to_writer(&mut *stdin, value).map_err(|source| {
        RuntimeError::InvalidProtocol(format!("cannot encode initialize message: {source}"))
    })?;
    stdin
        .write_all(b"\n")
        .and_then(|()| stdin.flush())
        .map_err(|source| RuntimeError::Io {
            operation: "stdin write",
            source,
        })
}

fn stop_child(child: &mut Child, stdin: impl Write) {
    drop(stdin);
    let _ = child.kill();
    let _ = child.wait();
}

fn finish_child(child: &mut Child) {
    let deadline = Instant::now() + Duration::from_secs(1);
    while Instant::now() < deadline {
        match child.try_wait() {
            Ok(Some(_)) => return,
            Ok(None) => thread::sleep(Duration::from_millis(20)),
            Err(_) => break,
        }
    }
    let _ = child.kill();
    let _ = child.wait();
}

struct ManagedChild(Child);

impl Drop for ManagedChild {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

fn same_path(left: &Path, right: &Path) -> bool {
    let left = left.canonicalize().unwrap_or_else(|_| left.to_path_buf());
    let right = right.canonicalize().unwrap_or_else(|_| right.to_path_buf());
    left == right
}

#[derive(Deserialize)]
struct InitializeEnvelope {
    id: Value,
    result: Option<InitializeResult>,
    error: Option<ProtocolError>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct InitializeResult {
    user_agent: String,
    codex_home: PathBuf,
    platform_family: String,
    platform_os: String,
}

#[derive(Deserialize)]
struct ProtocolError {
    code: Option<i64>,
    message: String,
}

fn parse_initialize_response(response: &str) -> Result<AppServerInfo, RuntimeError> {
    let envelope: InitializeEnvelope = serde_json::from_str(response).map_err(|source| {
        RuntimeError::InvalidProtocol(format!("initialize response is not JSON: {source}"))
    })?;
    if envelope.id != json!(1) {
        return Err(RuntimeError::InvalidProtocol(format!(
            "initialize response id was {}, expected 1",
            envelope.id
        )));
    }
    if let Some(error) = envelope.error {
        return Err(RuntimeError::Protocol {
            code: error.code,
            message: error.message,
        });
    }
    let result = envelope.result.ok_or_else(|| {
        RuntimeError::InvalidProtocol("initialize response has no result".to_string())
    })?;
    Ok(AppServerInfo {
        user_agent: result.user_agent,
        codex_home: result.codex_home,
        platform_family: result.platform_family,
        platform_os: result.platform_os,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

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
    fn parses_initialize_response() {
        let response = r#"{"id":1,"result":{"userAgent":"dscode-test","codexHome":"/tmp/dscode/codex","platformFamily":"unix","platformOs":"macos"}}"#;

        let info = parse_initialize_response(response).expect("valid response");

        assert_eq!(info.user_agent, "dscode-test");
        assert_eq!(info.codex_home, PathBuf::from("/tmp/dscode/codex"));
        assert_eq!(info.platform_family, "unix");
        assert_eq!(info.platform_os, "macos");
    }

    #[test]
    fn surfaces_initialize_protocol_errors() {
        let response = r#"{"id":1,"error":{"code":-32602,"message":"invalid params"}}"#;

        let error = parse_initialize_response(response).expect_err("protocol error");

        assert!(matches!(
            error,
            RuntimeError::Protocol {
                code: Some(-32602),
                ..
            }
        ));
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

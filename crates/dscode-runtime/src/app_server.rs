use std::collections::VecDeque;
use std::ffi::OsString;
use std::io::{self, BufRead, BufReader, Write};
use std::path::PathBuf;
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::{Arc, Mutex, mpsc};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use serde::Deserialize;
use serde_json::{Value, json};

use crate::{AppServerInfo, RuntimeError};

const STDERR_CAPTURE_LIMIT: usize = 64 * 1024;

pub struct AppServerClient {
    child: ManagedChild,
    stdin: Option<ChildStdin>,
    messages: mpsc::Receiver<Result<String, io::Error>>,
    pending: VecDeque<Value>,
    stderr: Arc<Mutex<String>>,
    stdout_reader: Option<JoinHandle<()>>,
    stderr_reader: Option<JoinHandle<()>>,
    next_request_id: i64,
}

impl AppServerClient {
    pub fn start(
        program: &str,
        args: &[String],
        environment: &[(OsString, OsString)],
        client_version: &str,
        timeout: Duration,
    ) -> Result<(Self, AppServerInfo), RuntimeError> {
        let child = Command::new(program)
            .args(args)
            .envs(environment.iter().cloned())
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|source| RuntimeError::Launch {
                program: program.to_string(),
                source,
            })?;
        let mut child = ManagedChild::new(child);
        let stdin = child.child_mut().stdin.take().ok_or_else(|| {
            RuntimeError::InvalidProtocol("stdin pipe is unavailable".to_string())
        })?;
        let stdout = child.child_mut().stdout.take().ok_or_else(|| {
            RuntimeError::InvalidProtocol("stdout pipe is unavailable".to_string())
        })?;
        let stderr = child.child_mut().stderr.take().ok_or_else(|| {
            RuntimeError::InvalidProtocol("stderr pipe is unavailable".to_string())
        })?;

        let (sender, messages) = mpsc::channel();
        let stdout_reader = thread::spawn(move || {
            let mut reader = BufReader::new(stdout);
            loop {
                let mut line = String::new();
                match reader.read_line(&mut line) {
                    Ok(0) => break,
                    Ok(_) => {
                        if sender.send(Ok(line)).is_err() {
                            break;
                        }
                    }
                    Err(error) => {
                        let _ = sender.send(Err(error));
                        break;
                    }
                }
            }
        });

        let stderr_capture = Arc::new(Mutex::new(String::new()));
        let stderr_output = Arc::clone(&stderr_capture);
        let stderr_reader = thread::spawn(move || {
            for line in BufReader::new(stderr).lines() {
                let Ok(line) = line else {
                    break;
                };
                let mut capture = stderr_output
                    .lock()
                    .unwrap_or_else(|error| error.into_inner());
                capture.push_str(&line);
                capture.push('\n');
                if capture.len() > STDERR_CAPTURE_LIMIT {
                    let overflow = capture.len() - STDERR_CAPTURE_LIMIT;
                    let boundary = capture
                        .char_indices()
                        .find_map(|(index, _)| (index >= overflow).then_some(index))
                        .unwrap_or(capture.len());
                    capture.drain(..boundary);
                }
            }
        });

        let mut client = Self {
            child,
            stdin: Some(stdin),
            messages,
            pending: VecDeque::new(),
            stderr: stderr_capture,
            stdout_reader: Some(stdout_reader),
            stderr_reader: Some(stderr_reader),
            next_request_id: 1,
        };
        let result = client
            .request(
                "initialize",
                json!({
                    "clientInfo": {
                        "name": "dscode",
                        "title": "DS Code",
                        "version": client_version
                    }
                }),
                timeout,
            )
            .map_err(|error| match error {
                RuntimeError::RequestTimeout { .. } => RuntimeError::ProbeTimeout(timeout),
                other => other,
            })?;
        let info: InitializeResult = serde_json::from_value(result).map_err(|source| {
            RuntimeError::InvalidProtocol(format!("cannot decode initialize result: {source}"))
        })?;
        client.notify("initialized", None)?;

        Ok((
            client,
            AppServerInfo {
                user_agent: info.user_agent,
                codex_home: info.codex_home,
                platform_family: info.platform_family,
                platform_os: info.platform_os,
            },
        ))
    }

    pub fn request(
        &mut self,
        method: &str,
        params: Value,
        timeout: Duration,
    ) -> Result<Value, RuntimeError> {
        let id = self.next_request_id;
        self.next_request_id = self.next_request_id.checked_add(1).unwrap_or(1);
        self.write_message(&json!({
            "method": method,
            "id": id,
            "params": params
        }))?;

        let deadline = Instant::now() + timeout;
        loop {
            let remaining = deadline.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                return Err(RuntimeError::RequestTimeout {
                    method: method.to_string(),
                    timeout,
                });
            }
            let Some(message) = self.receive_message(remaining)? else {
                return Err(RuntimeError::RequestTimeout {
                    method: method.to_string(),
                    timeout,
                });
            };
            let is_response =
                message.get("method").is_none() && message.get("id") == Some(&json!(id));
            if !is_response {
                self.pending.push_back(message);
                continue;
            }
            if let Some(error) = message.get("error") {
                let error: ProtocolError =
                    serde_json::from_value(error.clone()).map_err(|source| {
                        RuntimeError::InvalidProtocol(format!(
                            "cannot decode error response: {source}"
                        ))
                    })?;
                return Err(RuntimeError::Protocol {
                    code: error.code,
                    message: error.message,
                });
            }
            return message.get("result").cloned().ok_or_else(|| {
                RuntimeError::InvalidProtocol(format!(
                    "response to `{method}` has neither result nor error"
                ))
            });
        }
    }

    pub fn notify(&mut self, method: &str, params: Option<Value>) -> Result<(), RuntimeError> {
        let mut message = json!({ "method": method });
        if let Some(params) = params {
            message["params"] = params;
        }
        self.write_message(&message)
    }

    pub fn next_message(&mut self, timeout: Duration) -> Result<Option<Value>, RuntimeError> {
        if let Some(message) = self.pending.pop_front() {
            return Ok(Some(message));
        }
        self.receive_message(timeout)
    }

    pub fn pending_len(&self) -> usize {
        self.pending.len()
    }

    fn receive_message(&self, timeout: Duration) -> Result<Option<Value>, RuntimeError> {
        match self.messages.recv_timeout(timeout) {
            Ok(Ok(line)) => parse_message(&line).map(Some),
            Ok(Err(source)) => Err(RuntimeError::Io {
                operation: "stdout read",
                source,
            }),
            Err(mpsc::RecvTimeoutError::Timeout) => Ok(None),
            Err(mpsc::RecvTimeoutError::Disconnected) => Err(self.disconnected_error()),
        }
    }

    fn write_message(&mut self, value: &Value) -> Result<(), RuntimeError> {
        let stdin = self.stdin.as_mut().ok_or_else(|| {
            RuntimeError::InvalidProtocol("app-server stdin is closed".to_string())
        })?;
        serde_json::to_writer(&mut *stdin, value).map_err(|source| {
            RuntimeError::InvalidProtocol(format!("cannot encode app-server message: {source}"))
        })?;
        stdin
            .write_all(b"\n")
            .and_then(|()| stdin.flush())
            .map_err(|source| RuntimeError::Io {
                operation: "stdin write",
                source,
            })
    }

    fn disconnected_error(&self) -> RuntimeError {
        let stderr = self
            .stderr
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .trim()
            .to_string();
        RuntimeError::Disconnected { stderr }
    }
}

impl Drop for AppServerClient {
    fn drop(&mut self) {
        self.stdin.take();
        self.child.finish(Duration::from_secs(1));
        if let Some(reader) = self.stdout_reader.take() {
            let _ = reader.join();
        }
        if let Some(reader) = self.stderr_reader.take() {
            let _ = reader.join();
        }
    }
}

struct ManagedChild {
    child: Child,
    finished: bool,
}

impl ManagedChild {
    fn new(child: Child) -> Self {
        Self {
            child,
            finished: false,
        }
    }

    fn child_mut(&mut self) -> &mut Child {
        &mut self.child
    }

    fn finish(&mut self, grace_period: Duration) {
        if self.finished {
            return;
        }
        let deadline = Instant::now() + grace_period;
        while Instant::now() < deadline {
            match self.child.try_wait() {
                Ok(Some(_)) => {
                    self.finished = true;
                    return;
                }
                Ok(None) => thread::sleep(Duration::from_millis(20)),
                Err(_) => break,
            }
        }
        let _ = self.child.kill();
        let _ = self.child.wait();
        self.finished = true;
    }
}

impl Drop for ManagedChild {
    fn drop(&mut self) {
        self.finish(Duration::ZERO);
    }
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

fn parse_message(line: &str) -> Result<Value, RuntimeError> {
    serde_json::from_str(line).map_err(|source| {
        RuntimeError::InvalidProtocol(format!("app-server message is not JSON: {source}"))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_protocol_messages() {
        let message =
            parse_message(r#"{"id":1,"result":{"data":[]}}"#).expect("valid protocol message");

        assert_eq!(message["id"], 1);
        assert!(message["result"]["data"].is_array());
    }

    #[test]
    fn rejects_non_json_protocol_messages() {
        let error = parse_message("not-json").expect_err("invalid protocol message");

        assert!(matches!(error, RuntimeError::InvalidProtocol(_)));
    }
}

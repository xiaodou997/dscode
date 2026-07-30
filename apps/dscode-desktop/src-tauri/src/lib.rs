use std::env;
#[cfg(target_os = "windows")]
use std::ffi::OsStr;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use dscode_core::official_codex::{
    OfficialCodexConfigChange, OfficialCodexConfigManager, OfficialCodexConfigStatus,
};
use dscode_core::{DEFAULT_API_KEY_ENV, DEFAULT_BASE_URL, PROVIDER_ID};
use dscode_credentials::{CredentialStore, SystemCredentialStore};
use dscode_runtime::{TESTED_CODEX_VERSION, inspect_runtime};
use serde::Serialize;

const OFFICIAL_DOWNLOAD_URL: &str = "https://openai.com/codex/get-started/";

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct DashboardState {
    official_app: OfficialAppState,
    official_auth_present: bool,
    codex_home: String,
    endpoint: String,
    provider: ProviderState,
    credential_saved: bool,
    credential_error: Option<String>,
    local_runtime: LocalRuntimeState,
    latest_backup: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct OfficialAppState {
    installed: bool,
    running: bool,
    path: Option<String>,
    version: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct ProviderState {
    active_provider: Option<String>,
    doustack_configured: bool,
    doustack_active: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct LocalRuntimeState {
    available: bool,
    version: Option<String>,
    tested: bool,
    tested_version: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct ConfigMutationResult {
    state: DashboardState,
    changed: bool,
    config_path: String,
    backup_path: Option<String>,
}

#[tauri::command]
fn dashboard_state() -> Result<DashboardState, String> {
    build_dashboard_state()
}

#[tauri::command]
fn configure_doustack(api_key: String) -> Result<ConfigMutationResult, String> {
    let api_key = api_key.trim();
    let app = detect_official_app();
    if app.running {
        return Err("Quit the official Codex app before changing its configuration".to_string());
    }

    if api_key.is_empty() {
        let credential = SystemCredentialStore
            .load()
            .map_err(|error| error.to_string())?;
        if credential.is_none() {
            return Err("Enter a DouStack API key before enabling the provider".to_string());
        }
    } else {
        SystemCredentialStore
            .save(api_key)
            .map_err(|error| error.to_string())?;
    }
    let change = config_manager()?
        .apply_doustack_config()
        .map_err(|error| error.to_string())?;
    mutation_result(change)
}

#[tauri::command]
fn restore_latest_config() -> Result<ConfigMutationResult, String> {
    let app = detect_official_app();
    if app.running {
        return Err("Quit the official Codex app before restoring its configuration".to_string());
    }
    let change = config_manager()?
        .restore_latest_backup()
        .map_err(|error| error.to_string())?;
    mutation_result(change)
}

#[tauri::command]
fn forget_doustack_key() -> Result<DashboardState, String> {
    SystemCredentialStore
        .delete()
        .map_err(|error| error.to_string())?;
    build_dashboard_state()
}

#[tauri::command]
fn preview_doustack_config() -> Result<String, String> {
    config_manager()?
        .preview_doustack_config()
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn launch_official_codex() -> Result<(), String> {
    let app = detect_official_app();
    let app_path = app
        .path
        .as_deref()
        .map(PathBuf::from)
        .ok_or_else(|| "The official Codex app is not installed".to_string())?;

    if app.running {
        return activate_app(&app_path);
    }

    let status = config_manager()?
        .status()
        .map_err(|error| error.to_string())?;
    let credential = if status.active_provider.as_deref() == Some(PROVIDER_ID) {
        Some(
            SystemCredentialStore
                .load()
                .map_err(|error| error.to_string())?
                .ok_or_else(|| "No DouStack API key is saved".to_string())?,
        )
    } else {
        None
    };
    let executable = app_executable(&app_path)
        .ok_or_else(|| format!("Cannot find the executable in {}", app_path.display()))?;
    let mut command = Command::new(&executable);
    command
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    if let Some(credential) = credential {
        command.env(DEFAULT_API_KEY_ENV, credential);
    }
    command
        .spawn()
        .map_err(|error| format!("Cannot launch {}: {error}", executable.display()))?;
    Ok(())
}

#[tauri::command]
fn open_official_download() -> Result<(), String> {
    open_target(OFFICIAL_DOWNLOAD_URL)
}

fn mutation_result(change: OfficialCodexConfigChange) -> Result<ConfigMutationResult, String> {
    Ok(ConfigMutationResult {
        state: build_dashboard_state()?,
        changed: change.changed,
        config_path: change.config_path.display().to_string(),
        backup_path: change.backup_path.map(|path| path.display().to_string()),
    })
}

fn build_dashboard_state() -> Result<DashboardState, String> {
    let codex_home = official_codex_home()?;
    let config_status = config_manager()?
        .status()
        .map_err(|error| error.to_string())?;
    let (credential_saved, credential_error) = match SystemCredentialStore.load() {
        Ok(credential) => (credential.is_some(), None),
        Err(error) => (false, Some(error.to_string())),
    };
    let local_runtime = match inspect_runtime("codex") {
        Ok(runtime) => LocalRuntimeState {
            available: true,
            version: Some(runtime.version.to_string()),
            tested: runtime.compatibility.is_tested(),
            tested_version: TESTED_CODEX_VERSION.to_string(),
        },
        Err(_) => LocalRuntimeState {
            available: false,
            version: None,
            tested: false,
            tested_version: TESTED_CODEX_VERSION.to_string(),
        },
    };

    Ok(DashboardState {
        official_app: detect_official_app(),
        official_auth_present: official_auth_present(&codex_home),
        codex_home: codex_home.display().to_string(),
        endpoint: DEFAULT_BASE_URL.to_string(),
        provider: provider_state(&config_status),
        credential_saved,
        credential_error,
        local_runtime,
        latest_backup: config_status
            .latest_backup
            .map(|path| path.display().to_string()),
    })
}

fn provider_state(status: &OfficialCodexConfigStatus) -> ProviderState {
    ProviderState {
        active_provider: status.active_provider.clone(),
        doustack_configured: status.doustack_configured,
        doustack_active: status.active_provider.as_deref() == Some(PROVIDER_ID),
    }
}

fn config_manager() -> Result<OfficialCodexConfigManager, String> {
    let home = user_home()?;
    Ok(OfficialCodexConfigManager::new(
        home.join(".codex"),
        home.join(".dscode/backups/official-codex"),
    ))
}

fn official_codex_home() -> Result<PathBuf, String> {
    Ok(user_home()?.join(".codex"))
}

fn user_home() -> Result<PathBuf, String> {
    env::var_os("HOME")
        .or_else(|| env::var_os("USERPROFILE"))
        .filter(|home| !home.is_empty())
        .map(PathBuf::from)
        .ok_or_else(|| "Cannot resolve the user home directory".to_string())
}

fn official_auth_present(codex_home: &Path) -> bool {
    let Ok(contents) = fs::read_to_string(codex_home.join("auth.json")) else {
        return false;
    };
    let Ok(value) = serde_json::from_str::<serde_json::Value>(&contents) else {
        return false;
    };
    json_contains_chatgpt_auth(&value)
}

fn json_contains_chatgpt_auth(value: &serde_json::Value) -> bool {
    match value {
        serde_json::Value::Object(object) => object.iter().any(|(key, value)| {
            (key.eq_ignore_ascii_case("auth_mode")
                && value.as_str().is_some_and(|mode| {
                    mode.eq_ignore_ascii_case("chatgpt")
                        || mode.eq_ignore_ascii_case("chatgpt_auth_tokens")
                }))
                || (key.eq_ignore_ascii_case("access_token")
                    && value.as_str().is_some_and(|token| !token.is_empty()))
                || json_contains_chatgpt_auth(value)
        }),
        serde_json::Value::Array(values) => values.iter().any(json_contains_chatgpt_auth),
        _ => false,
    }
}

fn detect_official_app() -> OfficialAppState {
    let path = official_app_candidates()
        .into_iter()
        .find(|path| path.exists());
    let running = path.as_deref().is_some_and(is_app_running);
    let version = path.as_deref().and_then(app_version);
    OfficialAppState {
        installed: path.is_some(),
        running,
        path: path.map(|path| path.display().to_string()),
        version,
    }
}

#[cfg(target_os = "macos")]
fn official_app_candidates() -> Vec<PathBuf> {
    let mut candidates = vec![
        PathBuf::from("/Applications/ChatGPT.app"),
        PathBuf::from("/Applications/Codex.app"),
    ];
    if let Ok(home) = user_home() {
        candidates.push(home.join("Applications/ChatGPT.app"));
        candidates.push(home.join("Applications/Codex.app"));
    }
    candidates
}

#[cfg(target_os = "windows")]
fn official_app_candidates() -> Vec<PathBuf> {
    let mut candidates = Vec::new();
    if let Some(local_app_data) = env::var_os("LOCALAPPDATA") {
        let local = PathBuf::from(local_app_data);
        candidates.push(local.join("Programs/OpenAI Codex/Codex.exe"));
        candidates.push(local.join("OpenAI/Codex/Codex.exe"));
    }
    candidates
}

#[cfg(not(any(target_os = "macos", target_os = "windows")))]
fn official_app_candidates() -> Vec<PathBuf> {
    Vec::new()
}

#[cfg(target_os = "macos")]
fn app_executable(app_path: &Path) -> Option<PathBuf> {
    ["ChatGPT", "Codex"]
        .into_iter()
        .map(|name| app_path.join("Contents/MacOS").join(name))
        .find(|path| path.is_file())
}

#[cfg(target_os = "windows")]
fn app_executable(app_path: &Path) -> Option<PathBuf> {
    app_path.is_file().then(|| app_path.to_path_buf())
}

#[cfg(not(any(target_os = "macos", target_os = "windows")))]
fn app_executable(_app_path: &Path) -> Option<PathBuf> {
    None
}

#[cfg(target_os = "macos")]
fn app_version(app_path: &Path) -> Option<String> {
    let output = Command::new("/usr/bin/plutil")
        .args(["-extract", "CFBundleShortVersionString", "raw", "-o", "-"])
        .arg(app_path.join("Contents/Info.plist"))
        .output()
        .ok()?;
    output
        .status
        .success()
        .then(|| String::from_utf8_lossy(&output.stdout).trim().to_string())
}

#[cfg(not(target_os = "macos"))]
fn app_version(_app_path: &Path) -> Option<String> {
    None
}

#[cfg(target_os = "macos")]
fn is_app_running(app_path: &Path) -> bool {
    let Some(executable) = app_executable(app_path) else {
        return false;
    };
    let Ok(output) = Command::new("/bin/ps")
        .args(["-ax", "-o", "command="])
        .output()
    else {
        return false;
    };
    let needle = executable.to_string_lossy();
    String::from_utf8_lossy(&output.stdout)
        .lines()
        .any(|line| line.contains(needle.as_ref()))
}

#[cfg(target_os = "windows")]
fn is_app_running(app_path: &Path) -> bool {
    let Some(file_name) = app_path.file_name().and_then(OsStr::to_str) else {
        return false;
    };
    let Ok(output) = Command::new("tasklist")
        .args(["/FI", &format!("IMAGENAME eq {file_name}")])
        .output()
    else {
        return false;
    };
    String::from_utf8_lossy(&output.stdout)
        .to_ascii_lowercase()
        .contains(&file_name.to_ascii_lowercase())
}

#[cfg(not(any(target_os = "macos", target_os = "windows")))]
fn is_app_running(_app_path: &Path) -> bool {
    false
}

#[cfg(target_os = "macos")]
fn activate_app(app_path: &Path) -> Result<(), String> {
    let status = Command::new("/usr/bin/open")
        .arg("-a")
        .arg(app_path)
        .status()
        .map_err(|error| format!("Cannot activate {}: {error}", app_path.display()))?;
    status
        .success()
        .then_some(())
        .ok_or_else(|| format!("Cannot activate {}", app_path.display()))
}

#[cfg(not(target_os = "macos"))]
fn activate_app(app_path: &Path) -> Result<(), String> {
    let executable = app_executable(app_path)
        .ok_or_else(|| format!("Cannot find the executable in {}", app_path.display()))?;
    Command::new(&executable)
        .spawn()
        .map(|_| ())
        .map_err(|error| format!("Cannot activate {}: {error}", executable.display()))
}

#[cfg(target_os = "macos")]
fn open_target(target: &str) -> Result<(), String> {
    let status = Command::new("/usr/bin/open")
        .arg(target)
        .status()
        .map_err(|error| format!("Cannot open {target}: {error}"))?;
    status
        .success()
        .then_some(())
        .ok_or_else(|| format!("Cannot open {target}"))
}

#[cfg(target_os = "windows")]
fn open_target(target: &str) -> Result<(), String> {
    Command::new("cmd")
        .args(["/C", "start", "", target])
        .spawn()
        .map(|_| ())
        .map_err(|error| format!("Cannot open {target}: {error}"))
}

#[cfg(not(any(target_os = "macos", target_os = "windows")))]
fn open_target(_target: &str) -> Result<(), String> {
    Err("The official Codex app is not available on this platform".to_string())
}

pub fn run() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![
            dashboard_state,
            configure_doustack,
            restore_latest_config,
            forget_doustack_key,
            preview_doustack_config,
            launch_official_codex,
            open_official_download
        ])
        .run(tauri::generate_context!())
        .expect("error while running DS Code");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_nested_chatgpt_auth_without_exposing_it() {
        let value = serde_json::json!({
            "tokens": {
                "access_token": "secret-value"
            }
        });

        assert!(json_contains_chatgpt_auth(&value));
    }

    #[test]
    fn rejects_empty_auth_documents() {
        assert!(!json_contains_chatgpt_auth(&serde_json::json!({})));
    }
}

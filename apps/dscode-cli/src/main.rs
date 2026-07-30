use std::env;
use std::path::PathBuf;
use std::process::{Command, ExitCode, Stdio};

use dscode_core::config::AppConfig;
use dscode_core::data_layout::DataLayout;
use dscode_core::{
    CodexRuntime, DEFAULT_API_KEY_ENV, DEFAULT_CODEX_BINARY, LaunchRequest, ProviderSettings,
};
use dscode_credentials::{CredentialStore, SystemCredentialStore};

const VERSION: &str = env!("CARGO_PKG_VERSION");

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Action {
    Launch,
    Config,
    Doctor,
    Init,
    Login,
    Logout,
    Help,
    Version,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Cli {
    action: Action,
    base_url: Option<String>,
    model: Option<String>,
    codex_binary: Option<String>,
    data_home: Option<String>,
    forwarded_args: Vec<String>,
}

fn main() -> ExitCode {
    match run() {
        Ok(code) => ExitCode::from(code),
        Err(error) => {
            eprintln!("dscode: {error}");
            ExitCode::FAILURE
        }
    }
}

fn run() -> Result<u8, String> {
    let cli = parse_args(env::args().skip(1))?;
    let codex_binary = cli
        .codex_binary
        .clone()
        .or_else(|| non_empty_env("DSCODE_CODEX_BIN"))
        .unwrap_or_else(|| DEFAULT_CODEX_BINARY.to_string());

    match cli.action {
        Action::Config => {
            let layout = data_layout(&cli)?;
            configure(&cli, &layout)
        }
        Action::Doctor => {
            let layout = data_layout(&cli)?;
            let config = load_config(&layout)?;
            let settings = settings_from(&cli, &config);
            doctor(&settings, &codex_binary, &layout, &SystemCredentialStore)
        }
        Action::Init => initialize(&data_layout(&cli)?),
        Action::Login => login(&data_layout(&cli)?, &SystemCredentialStore),
        Action::Logout => logout(&SystemCredentialStore),
        Action::Help => {
            print_help();
            Ok(0)
        }
        Action::Version => {
            println!("dscode {VERSION}");
            Ok(0)
        }
        Action::Launch => {
            let layout = data_layout(&cli)?;
            let config = load_config(&layout)?;
            let settings = settings_from(&cli, &config);
            launch(
                settings,
                codex_binary,
                cli.forwarded_args,
                &layout,
                &SystemCredentialStore,
            )
        }
    }
}

fn settings_from(cli: &Cli, config: &AppConfig) -> ProviderSettings {
    settings_from_values(
        cli,
        config,
        non_empty_env("DOUSTACK_BASE_URL"),
        non_empty_env("DOUSTACK_MODEL"),
    )
}

fn settings_from_values(
    cli: &Cli,
    config: &AppConfig,
    environment_base_url: Option<String>,
    environment_model: Option<String>,
) -> ProviderSettings {
    ProviderSettings {
        base_url: cli
            .base_url
            .clone()
            .or(environment_base_url)
            .unwrap_or_else(|| config.provider.base_url.clone()),
        api_key_env: DEFAULT_API_KEY_ENV.to_string(),
        model: cli
            .model
            .clone()
            .or(environment_model)
            .or_else(|| config.provider.model.clone()),
    }
}

fn launch(
    settings: ProviderSettings,
    codex_binary: String,
    forwarded_args: Vec<String>,
    layout: &DataLayout,
    credential_store: &dyn CredentialStore,
) -> Result<u8, String> {
    let credential = resolve_credential(non_empty_env(DEFAULT_API_KEY_ENV), credential_store)?
        .ok_or_else(|| "no DouStack credential found; run `dscode login`".to_string())?;

    layout
        .ensure()
        .map_err(|error| format!("failed to initialize {}: {error}", layout.root().display()))?;

    let plan = CodexRuntime::plan(
        &settings,
        LaunchRequest {
            codex_binary,
            codex_home: layout.codex_home(),
            forwarded_args,
        },
    )
    .map_err(|error| error.to_string())?;

    let mut command = Command::new(&plan.program);
    command
        .args(&plan.args)
        .envs(plan.environment)
        .env(DEFAULT_API_KEY_ENV, &credential.value)
        .stdin(Stdio::inherit())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit());
    let status = command
        .status()
        .map_err(|error| format!("failed to start {}: {error}", plan.program))?;

    Ok(status.code().unwrap_or(1).clamp(0, u8::MAX as i32) as u8)
}

fn doctor(
    settings: &ProviderSettings,
    codex_binary: &str,
    layout: &DataLayout,
    credential_store: &dyn CredentialStore,
) -> Result<u8, String> {
    CodexRuntime::plan(
        settings,
        LaunchRequest {
            codex_binary: codex_binary.to_string(),
            codex_home: layout.codex_home(),
            forwarded_args: Vec::new(),
        },
    )
    .map_err(|error| error.to_string())?;

    println!("DS Code doctor");
    println!("[ok] provider: DouStack");
    println!("[ok] endpoint: {}", settings.base_url.trim_end_matches('/'));
    if layout.root().is_dir() {
        println!("[ok] data home: {}", layout.root().display());
    } else {
        println!(
            "[info] data home: {} (created by `dscode init` or on first launch)",
            layout.root().display()
        );
    }
    if let Some(model) = settings.model.as_deref() {
        println!("[ok] model: {model}");
    } else {
        println!("[info] model: Codex runtime default");
    }

    let mut healthy = true;
    match Command::new(codex_binary).arg("--version").output() {
        Ok(output) if output.status.success() => {
            let version = String::from_utf8_lossy(&output.stdout);
            println!("[ok] codex: {}", version.trim());
        }
        Ok(output) => {
            healthy = false;
            println!("[error] codex exited with {}", output.status);
        }
        Err(error) => {
            healthy = false;
            println!("[error] codex binary `{codex_binary}` is unavailable: {error}");
        }
    }

    match resolve_credential(non_empty_env(DEFAULT_API_KEY_ENV), credential_store) {
        Ok(Some(credential)) => println!(
            "[ok] credential: {} (value hidden)",
            credential.source.description()
        ),
        Ok(None) => {
            healthy = false;
            println!("[error] credential: not found; run `dscode login`");
        }
        Err(error) => {
            healthy = false;
            println!("[error] credential: {error}");
        }
    }

    if healthy { Ok(0) } else { Ok(1) }
}

fn initialize(layout: &DataLayout) -> Result<u8, String> {
    layout
        .ensure()
        .map_err(|error| format!("failed to initialize {}: {error}", layout.root().display()))?;
    if !layout.config_file().exists() {
        AppConfig::default()
            .save(&layout.config_file())
            .map_err(|error| error.to_string())?;
    }
    println!("DS Code home initialized: {}", layout.root().display());
    println!("Codex runtime home: {}", layout.codex_home().display());
    println!("Configuration: {}", layout.config_file().display());
    Ok(0)
}

fn login(layout: &DataLayout, credential_store: &dyn CredentialStore) -> Result<u8, String> {
    layout
        .ensure()
        .map_err(|error| format!("failed to initialize {}: {error}", layout.root().display()))?;
    let credential = rpassword::prompt_password("DouStack API key: ")
        .map_err(|error| format!("cannot read API key: {error}"))?;
    let credential = credential.trim();
    credential_store
        .save(credential)
        .map_err(|error| error.to_string())?;
    println!("DouStack credential saved in the system credential store.");
    Ok(0)
}

fn logout(credential_store: &dyn CredentialStore) -> Result<u8, String> {
    let deleted = credential_store
        .delete()
        .map_err(|error| error.to_string())?;
    if deleted {
        println!("DouStack credential removed from the system credential store.");
    } else {
        println!("No stored DouStack credential was found.");
    }
    if non_empty_env(DEFAULT_API_KEY_ENV).is_some() {
        println!("Note: {DEFAULT_API_KEY_ENV} is still set in this environment.");
    }
    Ok(0)
}

fn configure(cli: &Cli, layout: &DataLayout) -> Result<u8, String> {
    let mut config = load_config(layout)?;
    match cli.forwarded_args.as_slice() {
        [] => {
            let rendered = CodexRuntime::render_config(&settings_from(cli, &config))
                .map_err(|error| error.to_string())?;
            print!("{rendered}");
        }
        [value] if value == "show" => {
            let rendered = CodexRuntime::render_config(&settings_from(cli, &config))
                .map_err(|error| error.to_string())?;
            print!("{rendered}");
        }
        [command, key, value] if command == "set" && key == "model" => {
            config.provider.model = Some(value.clone());
            save_config(layout, &config)?;
            println!(
                "Saved model `{value}` to {}",
                layout.config_file().display()
            );
        }
        [command, key, value] if command == "set" && key == "base-url" => {
            config.provider.base_url = value.trim_end_matches('/').to_string();
            save_config(layout, &config)?;
            println!(
                "Saved endpoint `{}` to {}",
                config.provider.base_url,
                layout.config_file().display()
            );
        }
        [command, key] if command == "unset" && key == "model" => {
            config.provider.model = None;
            save_config(layout, &config)?;
            println!(
                "Removed the saved model from {}",
                layout.config_file().display()
            );
        }
        [value] if value == "path" => println!("{}", layout.config_file().display()),
        _ => {
            return Err(
                "usage: dscode config [show|path|set model MODEL|set base-url URL|unset model]"
                    .to_string(),
            );
        }
    }
    Ok(0)
}

fn load_config(layout: &DataLayout) -> Result<AppConfig, String> {
    AppConfig::load(&layout.config_file()).map_err(|error| error.to_string())
}

fn save_config(layout: &DataLayout, config: &AppConfig) -> Result<(), String> {
    layout
        .ensure()
        .map_err(|error| format!("failed to initialize {}: {error}", layout.root().display()))?;
    config
        .save(&layout.config_file())
        .map_err(|error| error.to_string())
}

struct ResolvedCredential {
    value: String,
    source: CredentialSource,
}

#[derive(Clone, Copy)]
enum CredentialSource {
    Environment,
    SystemStore,
}

impl CredentialSource {
    fn description(self) -> &'static str {
        match self {
            Self::Environment => "DOUSTACK_API_KEY environment override",
            Self::SystemStore => "system credential store",
        }
    }
}

fn resolve_credential(
    environment_value: Option<String>,
    credential_store: &dyn CredentialStore,
) -> Result<Option<ResolvedCredential>, String> {
    if let Some(value) = environment_value {
        return Ok(Some(ResolvedCredential {
            value,
            source: CredentialSource::Environment,
        }));
    }
    credential_store
        .load()
        .map(|credential| {
            credential.map(|value| ResolvedCredential {
                value,
                source: CredentialSource::SystemStore,
            })
        })
        .map_err(|error| error.to_string())
}

fn parse_args(args: impl IntoIterator<Item = String>) -> Result<Cli, String> {
    let mut cli = Cli {
        action: Action::Launch,
        base_url: None,
        model: None,
        codex_binary: None,
        data_home: None,
        forwarded_args: Vec::new(),
    };
    let mut values = args.into_iter().peekable();
    let mut command_position = true;

    while let Some(value) = values.next() {
        if value == "--" {
            cli.forwarded_args.extend(values);
            break;
        }

        match value.as_str() {
            "--base-url" => cli.base_url = Some(next_value(&mut values, "--base-url")?),
            "--model" => cli.model = Some(next_value(&mut values, "--model")?),
            "--codex" => cli.codex_binary = Some(next_value(&mut values, "--codex")?),
            "--home" => cli.data_home = Some(next_value(&mut values, "--home")?),
            "-h" | "--help" if command_position => cli.action = Action::Help,
            "-V" | "--version" if command_position => cli.action = Action::Version,
            "config" if command_position => cli.action = Action::Config,
            "doctor" if command_position => cli.action = Action::Doctor,
            "init" if command_position => cli.action = Action::Init,
            "login" if command_position => cli.action = Action::Login,
            "logout" if command_position => cli.action = Action::Logout,
            "help" if command_position => cli.action = Action::Help,
            other => {
                cli.forwarded_args.push(other.to_string());
                command_position = false;
            }
        }
        if cli.action != Action::Launch {
            command_position = false;
        }
    }

    if !matches!(cli.action, Action::Launch | Action::Config) && !cli.forwarded_args.is_empty() {
        return Err("this DS Code command does not accept Codex arguments".to_string());
    }
    Ok(cli)
}

fn next_value(values: &mut impl Iterator<Item = String>, option: &str) -> Result<String, String> {
    values
        .next()
        .filter(|value| !value.is_empty())
        .ok_or_else(|| format!("{option} requires a value"))
}

fn non_empty_env(name: &str) -> Option<String> {
    env::var(name).ok().filter(|value| !value.trim().is_empty())
}

fn data_layout(cli: &Cli) -> Result<DataLayout, String> {
    data_layout_from_values(cli.data_home.clone())
}

fn data_layout_from_values(cli_home: Option<String>) -> Result<DataLayout, String> {
    let root = cli_home
        .map(PathBuf::from)
        .or_else(|| {
            env::var_os("DSCODE_HOME")
                .filter(|value| !value.is_empty())
                .map(PathBuf::from)
        })
        .or_else(default_data_root)
        .ok_or_else(|| "cannot determine user home; set DSCODE_HOME or pass --home".to_string())?;
    let absolute_root = if root.is_absolute() {
        root
    } else {
        env::current_dir()
            .map_err(|error| format!("cannot resolve DS Code home: {error}"))?
            .join(root)
    };
    DataLayout::new(absolute_root).map_err(|error| error.to_string())
}

fn default_data_root() -> Option<PathBuf> {
    #[cfg(windows)]
    let home = env::var_os("USERPROFILE");
    #[cfg(not(windows))]
    let home = env::var_os("HOME");

    home.filter(|value| !value.is_empty())
        .map(PathBuf::from)
        .map(|path| path.join(".dscode"))
}

fn print_help() {
    println!(
        "DS Code {VERSION}\n\
         \n\
         Usage:\n\
           dscode [OPTIONS] [CODEX_ARGS...]\n\
           dscode config [show|path|set|unset] [OPTIONS]\n\
           dscode doctor [OPTIONS]\n\
           dscode init [OPTIONS]\n\
           dscode login [OPTIONS]\n\
           dscode logout [OPTIONS]\n\
         \n\
         Options:\n\
           --base-url URL   DouStack Responses API root\n\
           --model MODEL    Model passed to Codex\n\
           --codex PATH     Codex executable (default: codex)\n\
           --home PATH      DS Code data home (default: ~/.dscode)\n\
           -h, --help       Show help\n\
           -V, --version    Show version\n\
         \n\
         Environment:\n\
           DOUSTACK_API_KEY   Credential override; its value is never printed\n\
           DOUSTACK_BASE_URL  Endpoint override\n\
           DOUSTACK_MODEL     Default model override\n\
           DSCODE_HOME        DS Code data home override\n\
           DSCODE_CODEX_BIN   Codex executable override"
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    fn strings(values: &[&str]) -> Vec<String> {
        values.iter().map(|value| (*value).to_string()).collect()
    }

    #[test]
    fn unknown_subcommand_is_forwarded_to_codex() {
        let cli =
            parse_args(strings(&["exec", "inspect this repo"])).expect("valid forwarded command");

        assert_eq!(cli.action, Action::Launch);
        assert_eq!(cli.forwarded_args, strings(&["exec", "inspect this repo"]));
    }

    #[test]
    fn dscode_options_are_removed_from_forwarded_arguments() {
        let cli = parse_args(strings(&[
            "--base-url",
            "https://example.test/v1",
            "--model",
            "gpt-example",
            "exec",
            "hello",
        ]))
        .expect("valid options");

        assert_eq!(cli.base_url.as_deref(), Some("https://example.test/v1"));
        assert_eq!(cli.model.as_deref(), Some("gpt-example"));
        assert_eq!(cli.forwarded_args, strings(&["exec", "hello"]));
    }

    #[test]
    fn separator_forwards_all_remaining_arguments() {
        let cli =
            parse_args(strings(&["--", "--model", "codex-owned-model"])).expect("valid separator");

        assert_eq!(
            cli.forwarded_args,
            strings(&["--model", "codex-owned-model"])
        );
    }

    #[test]
    fn init_accepts_an_explicit_data_home() {
        let cli = parse_args(strings(&["init", "--home", "/tmp/dscode-test-home"]))
            .expect("valid init command");

        assert_eq!(cli.action, Action::Init);
        assert_eq!(cli.data_home.as_deref(), Some("/tmp/dscode-test-home"));
        assert!(cli.forwarded_args.is_empty());
    }

    #[test]
    fn command_can_follow_global_options() {
        let cli = parse_args(strings(&["--home", "/tmp/dscode-test-home", "doctor"]))
            .expect("valid doctor command");

        assert_eq!(cli.action, Action::Doctor);
        assert_eq!(cli.data_home.as_deref(), Some("/tmp/dscode-test-home"));
        assert!(cli.forwarded_args.is_empty());
    }

    #[test]
    fn login_is_a_dscode_command() {
        let cli = parse_args(strings(&["login", "--home", "/tmp/dscode-test-home"]))
            .expect("valid login command");

        assert_eq!(cli.action, Action::Login);
        assert!(cli.forwarded_args.is_empty());
    }

    #[test]
    fn config_accepts_setting_arguments() {
        let cli = parse_args(strings(&["config", "set", "model", "gpt-example"]))
            .expect("valid config command");

        assert_eq!(cli.action, Action::Config);
        assert_eq!(
            cli.forwarded_args,
            strings(&["set", "model", "gpt-example"])
        );
    }

    #[test]
    fn cli_settings_override_environment_and_saved_config() {
        let cli = parse_args(strings(&[
            "--base-url",
            "https://cli.example/v1",
            "--model",
            "cli-model",
        ]))
        .expect("valid CLI settings");
        let mut config = AppConfig::default();
        config.provider.base_url = "https://saved.example/v1".to_string();
        config.provider.model = Some("saved-model".to_string());

        let settings = settings_from_values(
            &cli,
            &config,
            Some("https://environment.example/v1".to_string()),
            Some("environment-model".to_string()),
        );

        assert_eq!(settings.base_url, "https://cli.example/v1");
        assert_eq!(settings.model.as_deref(), Some("cli-model"));
    }

    #[test]
    fn environment_settings_override_saved_config() {
        let cli = parse_args(Vec::<String>::new()).expect("default CLI settings");
        let mut config = AppConfig::default();
        config.provider.base_url = "https://saved.example/v1".to_string();
        config.provider.model = Some("saved-model".to_string());

        let settings = settings_from_values(
            &cli,
            &config,
            Some("https://environment.example/v1".to_string()),
            Some("environment-model".to_string()),
        );

        assert_eq!(settings.base_url, "https://environment.example/v1");
        assert_eq!(settings.model.as_deref(), Some("environment-model"));
    }

    #[derive(Default)]
    struct MemoryCredentialStore {
        value: Option<String>,
    }

    impl CredentialStore for MemoryCredentialStore {
        fn load(&self) -> Result<Option<String>, dscode_credentials::CredentialError> {
            Ok(self.value.clone())
        }

        fn save(&self, _credential: &str) -> Result<(), dscode_credentials::CredentialError> {
            unreachable!("not used by credential resolution tests")
        }

        fn delete(&self) -> Result<bool, dscode_credentials::CredentialError> {
            unreachable!("not used by credential resolution tests")
        }
    }

    #[test]
    fn environment_credential_takes_precedence() {
        let store = MemoryCredentialStore {
            value: Some("stored-value".to_string()),
        };

        let resolved = resolve_credential(Some("environment-value".to_string()), &store)
            .expect("resolve credential")
            .expect("credential exists");

        assert_eq!(resolved.value, "environment-value");
        assert!(matches!(resolved.source, CredentialSource::Environment));
    }

    #[test]
    fn system_store_is_used_without_an_environment_override() {
        let store = MemoryCredentialStore {
            value: Some("stored-value".to_string()),
        };

        let resolved = resolve_credential(None, &store)
            .expect("resolve credential")
            .expect("credential exists");

        assert_eq!(resolved.value, "stored-value");
        assert!(matches!(resolved.source, CredentialSource::SystemStore));
    }
}

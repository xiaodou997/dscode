use dscode_core::{
    CodexRuntime, DEFAULT_API_KEY_ENV, DEFAULT_BASE_URL, DEFAULT_CODEX_BINARY, LaunchRequest,
    ProviderSettings,
};
use std::env;
use std::path::PathBuf;
use std::process::{Command, ExitCode, Stdio};

use dscode_core::data_layout::DataLayout;

const VERSION: &str = env!("CARGO_PKG_VERSION");

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Action {
    Launch,
    Config,
    Doctor,
    Init,
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
    let settings = settings_from(&cli);
    let codex_binary = cli
        .codex_binary
        .clone()
        .or_else(|| non_empty_env("DSCODE_CODEX_BIN"))
        .unwrap_or_else(|| DEFAULT_CODEX_BINARY.to_string());

    match cli.action {
        Action::Config => {
            let config =
                CodexRuntime::render_config(&settings).map_err(|error| error.to_string())?;
            print!("{config}");
            Ok(0)
        }
        Action::Doctor => {
            let layout = data_layout(&cli)?;
            doctor(&settings, &codex_binary, &layout)
        }
        Action::Init => initialize(&data_layout(&cli)?),
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
            launch(settings, codex_binary, cli.forwarded_args, &layout)
        }
    }
}

fn settings_from(cli: &Cli) -> ProviderSettings {
    ProviderSettings {
        base_url: cli
            .base_url
            .clone()
            .or_else(|| non_empty_env("DOUSTACK_BASE_URL"))
            .unwrap_or_else(|| DEFAULT_BASE_URL.to_string()),
        api_key_env: DEFAULT_API_KEY_ENV.to_string(),
        model: cli
            .model
            .clone()
            .or_else(|| non_empty_env("DOUSTACK_MODEL")),
    }
}

fn launch(
    settings: ProviderSettings,
    codex_binary: String,
    forwarded_args: Vec<String>,
    layout: &DataLayout,
) -> Result<u8, String> {
    if non_empty_env(DEFAULT_API_KEY_ENV).is_none() {
        return Err(format!(
            "{DEFAULT_API_KEY_ENV} is not set; run `dscode doctor` for setup details"
        ));
    }

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

    let status = Command::new(&plan.program)
        .args(&plan.args)
        .envs(plan.environment)
        .stdin(Stdio::inherit())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .status()
        .map_err(|error| format!("failed to start {}: {error}", plan.program))?;

    Ok(status.code().unwrap_or(1).clamp(0, u8::MAX as i32) as u8)
}

fn doctor(
    settings: &ProviderSettings,
    codex_binary: &str,
    layout: &DataLayout,
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

    if non_empty_env(DEFAULT_API_KEY_ENV).is_some() {
        println!("[ok] credential: {DEFAULT_API_KEY_ENV} is set (value hidden)");
    } else {
        healthy = false;
        println!("[error] credential: {DEFAULT_API_KEY_ENV} is not set");
    }

    if healthy { Ok(0) } else { Ok(1) }
}

fn initialize(layout: &DataLayout) -> Result<u8, String> {
    layout
        .ensure()
        .map_err(|error| format!("failed to initialize {}: {error}", layout.root().display()))?;
    println!("DS Code home initialized: {}", layout.root().display());
    println!("Codex runtime home: {}", layout.codex_home().display());
    Ok(0)
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

    if cli.action != Action::Launch && !cli.forwarded_args.is_empty() {
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
           dscode config [OPTIONS]\n\
           dscode doctor [OPTIONS]\n\
           dscode init [OPTIONS]\n\
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
           DOUSTACK_API_KEY   Required credential; its value is never printed\n\
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
}

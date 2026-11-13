use std::collections::HashSet;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Child, Command as ProcessCommand, ExitCode};

use clap::{Arg, ArgAction, ArgMatches, Command};
use serde::Deserialize;

const PROGRAM_NAME: &str = "RunFast";
const PROGRAM_ALIAS: &str = "runf";
const PROGRAM_DESCRIPTION: &str = "Start configured programs quickly";
const PROGRAM_AUTHOR: &str = "CunZai1069 <cunzai1069@gmail.com>";
const PROGRAM_VERSION: &str = env!("CARGO_PKG_VERSION");
const CONFIG_ENV: &str = "RUNFAST_CONFIG";

#[derive(Debug, Deserialize)]
struct Config {
    #[serde(default)]
    item: Vec<ConfigItem>,
}

#[derive(Debug, Deserialize)]
struct ConfigItem {
    name: String,
    #[serde(default)]
    alias: Vec<String>,
    #[serde(default)]
    keywords: Vec<String>,
    #[serde(default)]
    path: Vec<PathBuf>,
    #[serde(default)]
    start_args: Vec<String>,
    /// Optional working directory. When left empty the launched program's own
    /// directory is used, matching a Windows shortcut's "Start in" behaviour.
    /// Some programs (e.g. OBS, which resolves `data\...\locale\*.ini`) only
    /// start correctly when the working directory is their own directory.
    #[serde(default)]
    work_dir: PathBuf,
    #[serde(default)]
    description: String,
}

fn cli() -> Command {
    Command::new(PROGRAM_ALIAS)
        .bin_name(PROGRAM_ALIAS)
        .display_name(PROGRAM_NAME)
        .about(PROGRAM_DESCRIPTION)
        .author(PROGRAM_AUTHOR)
        .version(PROGRAM_VERSION)
        .arg(
            Arg::new("config")
                .short('c')
                .long("config")
                .value_name("FILE")
                .help("Use a specific TOML configuration file"),
        )
        .arg(
            Arg::new("track")
                .short('t')
                .long("track")
                .help("Wait for launched processes and report their exit status")
                .action(ArgAction::SetTrue),
        )
        .arg(
            Arg::new("args")
                .long("args")
                .value_name("ARGS")
                .help(
                    "Extra arguments appended to the configured start_args; split on whitespace, \
                     so quote the whole string and use \"...\" inside it for paths containing spaces",
                )
                .action(ArgAction::Append),
        )
        .arg(
            Arg::new("path")
                .short('p')
                .long("path")
                .value_name("PROGRAM")
                .help("Launch a program directly; may be specified multiple times")
                .action(ArgAction::Append),
        )
        .arg(
            Arg::new("target")
                .value_name("TARGET")
                .help("Configured name or alias to launch")
                .num_args(1..)
                .action(ArgAction::Append),
        )
        .subcommand(
            Command::new("search")
                .short_flag('s')
                .long_flag("search")
                .about("Search configured programs")
                .arg(
                    Arg::new("keyword")
                        .value_name("KEYWORD")
                        .help("Match name, alias, keyword, path, or description")
                        .required(true)
                        .num_args(1..)
                        .action(ArgAction::Append),
                ),
        )
        .subcommand(
            Command::new("list")
                .short_flag('l')
                .long_flag("list")
                .about("List all configured programs"),
        )
        .after_help(format!(
            "Configuration lookup order: --config, {CONFIG_ENV}, executable directory, executable parent, current directory"
        ))
}

fn main() -> ExitCode {
    match run(cli().get_matches()) {
        Ok(()) => ExitCode::SUCCESS,
        Err(message) => {
            eprintln!("error: {message}");
            ExitCode::FAILURE
        }
    }
}

fn run(matches: ArgMatches) -> Result<(), String> {
    let track = matches.get_flag("track");

    match matches.subcommand() {
        Some(("list", _)) => {
            let (_, config) = load_config(matches.get_one::<String>("config"))?;
            list_items(&config.item);
            return Ok(());
        }
        Some(("search", sub_matches)) => {
            let (_, config) = load_config(matches.get_one::<String>("config"))?;
            let terms = sub_matches
                .get_many::<String>("keyword")
                .expect("clap requires at least one search keyword")
                .map(String::as_str)
                .collect::<Vec<_>>();
            let found = search_items(&config.item, &terms);
            if found.is_empty() {
                return Err(format!("no configuration matched: {}", terms.join(" ")));
            }
            list_items(&found);
            return Ok(());
        }
        Some((name, _)) => return Err(format!("unsupported command: {name}")),
        None => {}
    }

    let targets = matches
        .get_many::<String>("target")
        .map(|values| values.map(String::as_str).collect::<Vec<_>>())
        .unwrap_or_default();
    let direct_paths = matches
        .get_many::<String>("path")
        .map(|values| values.map(PathBuf::from).collect::<Vec<_>>())
        .unwrap_or_default();
    // `--args` may be supplied several times; each occurrence is split on
    // whitespace and appended in the order given, after the configured
    // `start_args` of every launched program.
    let extra_args = matches
        .get_many::<String>("args")
        .map(|values| {
            values
                .flat_map(|value| split_args(value))
                .collect::<Vec<String>>()
        })
        .unwrap_or_default();

    if targets.is_empty() && direct_paths.is_empty() {
        let mut command = cli();
        command.print_help().map_err(|error| error.to_string())?;
        println!();
        return Err("provide at least one TARGET or --path".to_owned());
    }

    let mut children = Vec::new();

    if !targets.is_empty() {
        let (config_path, config) = load_config(matches.get_one::<String>("config"))?;
        let config_dir = config_path.parent().unwrap_or_else(|| Path::new("."));

        for target in targets {
            let item = find_item(&config.item, target).ok_or_else(|| {
                format!("unknown target '{target}'; use 'runf list' to inspect entries")
            })?;
            let child = launch_item(item, config_dir, &extra_args)?;
            println!("started {} (PID {})", item.name, child.id());
            children.push((item.name.clone(), child));
        }
    }

    for path in direct_paths {
        let child = spawn_program(&path, &extra_args, None)
            .map_err(|error| format!("failed to start '{}': {error}", path.display()))?;
        println!("started {} (PID {})", path.display(), child.id());
        children.push((path.display().to_string(), child));
    }

    if track {
        wait_for_children(children)?;
    }

    Ok(())
}

fn load_config(explicit_path: Option<&String>) -> Result<(PathBuf, Config), String> {
    let path = locate_config(explicit_path)?;
    let content = fs::read_to_string(&path)
        .map_err(|error| format!("cannot read '{}': {error}", path.display()))?;
    let config = toml::from_str::<Config>(&content)
        .map_err(|error| format!("cannot parse '{}': {error}", path.display()))?;

    validate_config(&config)?;
    Ok((path, config))
}

fn locate_config(explicit_path: Option<&String>) -> Result<PathBuf, String> {
    if let Some(path) = explicit_path {
        return require_file(PathBuf::from(path), "--config");
    }

    if let Some(path) = env::var_os(CONFIG_ENV) {
        return require_file(PathBuf::from(path), CONFIG_ENV);
    }

    let mut candidates = Vec::new();
    if let Ok(executable) = env::current_exe()
        && let Some(executable_dir) = executable.parent()
    {
        candidates.push(executable_dir.join("config.toml"));
        if let Some(parent) = executable_dir.parent() {
            candidates.push(parent.join("config.toml"));
        }
    }
    if let Ok(current_dir) = env::current_dir() {
        candidates.push(current_dir.join("config.toml"));
    }

    let mut seen = HashSet::new();
    candidates.retain(|path| seen.insert(path.clone()));

    candidates
        .iter()
        .find(|path| path.is_file())
        .cloned()
        .ok_or_else(|| {
            let searched = candidates
                .iter()
                .map(|path| format!("'{}'", path.display()))
                .collect::<Vec<_>>()
                .join(", ");
            format!(
                "configuration file not found; use --config or {CONFIG_ENV} (searched {searched})"
            )
        })
}

fn require_file(path: PathBuf, source: &str) -> Result<PathBuf, String> {
    if path.is_file() {
        Ok(path)
    } else {
        Err(format!(
            "configuration from {source} does not exist or is not a file: '{}'",
            path.display()
        ))
    }
}

fn validate_config(config: &Config) -> Result<(), String> {
    let mut identifiers = HashSet::new();

    for item in &config.item {
        let name = item.name.trim();
        if name.is_empty() {
            return Err("configuration contains an item with an empty name".to_owned());
        }

        for identifier in std::iter::once(name).chain(
            item.alias
                .iter()
                .map(String::as_str)
                .map(str::trim)
                .filter(|alias| !alias.is_empty()),
        ) {
            let normalized = identifier.to_lowercase();
            if !identifiers.insert(normalized) {
                return Err(format!(
                    "duplicate name or alias in configuration: '{identifier}'"
                ));
            }
        }
    }

    Ok(())
}

fn find_item<'a>(items: &'a [ConfigItem], target: &str) -> Option<&'a ConfigItem> {
    let normalized = target.trim().to_lowercase();
    items.iter().find(|item| {
        item.name.trim().to_lowercase() == normalized
            || item
                .alias
                .iter()
                .any(|alias| alias.trim().to_lowercase() == normalized)
    })
}

fn search_items<'a>(items: &'a [ConfigItem], terms: &[&str]) -> Vec<&'a ConfigItem> {
    let terms = terms
        .iter()
        .map(|term| term.to_lowercase())
        .collect::<Vec<_>>();

    items
        .iter()
        .filter(|item| {
            let searchable = std::iter::once(item.name.as_str())
                .chain(item.alias.iter().map(String::as_str))
                .chain(item.keywords.iter().map(String::as_str))
                .chain(item.path.iter().filter_map(|path| path.to_str()))
                .chain(std::iter::once(item.description.as_str()))
                .collect::<Vec<_>>()
                .join("\n")
                .to_lowercase();
            terms.iter().all(|term| searchable.contains(term))
        })
        .collect()
}

fn list_items<T>(items: &[T])
where
    T: AsRef<ConfigItem>,
{
    if items.is_empty() {
        println!("No programs configured.");
        return;
    }

    for item in items {
        let item = item.as_ref();
        let aliases = item
            .alias
            .iter()
            .filter(|alias| !alias.trim().is_empty())
            .map(String::as_str)
            .collect::<Vec<_>>();
        let alias_text = if aliases.is_empty() {
            String::new()
        } else {
            format!(" [{}]", aliases.join(", "))
        };
        let description = if item.description.trim().is_empty() {
            String::new()
        } else {
            format!(" - {}", item.description.trim())
        };

        println!("{}{}{}", item.name, alias_text, description);
        for path in &item.path {
            println!("  {}", path.display());
        }
    }
}

impl AsRef<ConfigItem> for ConfigItem {
    fn as_ref(&self) -> &ConfigItem {
        self
    }
}

fn launch_item(
    item: &ConfigItem,
    config_dir: &Path,
    extra_args: &[String],
) -> Result<Child, String> {
    if item.path.is_empty() {
        return Err(format!("target '{}' has no program path", item.name));
    }

    let explicit_work_dir = if item.work_dir.as_os_str().is_empty() {
        None
    } else {
        Some(resolve_relative(config_dir, &item.work_dir))
    };
    if let Some(directory) = &explicit_work_dir
        && !directory.is_dir()
    {
        return Err(format!(
            "working directory for '{}' does not exist: '{}'",
            item.name,
            directory.display()
        ));
    }

    // Configured arguments first, then whatever `--args` supplied. A `--path`
    // launch has no configured arguments, so it receives only the extra ones.
    let mut arguments = item.start_args.clone();
    arguments.extend_from_slice(extra_args);

    let mut failures = Vec::new();
    for configured_path in &item.path {
        if configured_path.as_os_str().is_empty() {
            continue;
        }
        let program = resolve_program(config_dir, configured_path);
        let working_directory = resolve_work_dir(explicit_work_dir.clone(), &program);
        match spawn_program(&program, &arguments, working_directory.as_deref()) {
            Ok(child) => return Ok(child),
            Err(error) => failures.push(format!("'{}': {error}", program.display())),
        }
    }

    if failures.is_empty() {
        Err(format!("target '{}' has no usable program path", item.name))
    } else {
        Err(format!(
            "failed to start '{}'; tried {}",
            item.name,
            failures.join("; ")
        ))
    }
}

fn resolve_program(config_dir: &Path, program: &Path) -> PathBuf {
    if program.is_absolute() || program.components().count() == 1 {
        program.to_owned()
    } else {
        resolve_relative(config_dir, program)
    }
}

/// Working directory used for a launch.
///
/// An explicitly configured `work_dir` always wins. Otherwise the directory
/// holding the program is used — the same default a Windows shortcut applies to
/// its "Start in" field — which keeps programs that look up their own resources
/// (locales, plugins, themes) relative to their installation working even when
/// the launcher itself was started from somewhere else. Bare program names, which
/// rely on `PATH` lookup, have no directory of their own, so they keep inheriting
/// the launcher's working directory.
fn resolve_work_dir(explicit: Option<PathBuf>, program: &Path) -> Option<PathBuf> {
    explicit.or_else(|| {
        program
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
            .map(Path::to_path_buf)
    })
}

fn resolve_relative(base: &Path, path: &Path) -> PathBuf {
    if path.is_absolute() {
        path.to_owned()
    } else {
        base.join(path)
    }
}

/// Splits an `--args` value into individual program arguments.
///
/// Arguments are separated by whitespace. Double or single quotes group a value
/// that itself contains spaces, e.g.
/// `--args "-drive \"format=raw,file=D:/my os/bios.img\""` (as written in the
/// shell: `--args '-drive "format=raw,file=D:/my os/bios.img"'`).
/// Backslashes are never treated as escape characters, because Windows paths are
/// full of them and `D:\XiangMu\QH_OS` must reach the child process untouched.
/// An empty or whitespace-only value yields no arguments at all.
fn split_args(input: &str) -> Vec<String> {
    let mut args = Vec::new();
    let mut current = String::new();
    let mut quote: Option<char> = None;
    let mut in_token = false;

    for character in input.chars() {
        match quote {
            Some(active) if character == active => quote = None,
            Some(_) => current.push(character),
            None if character == '"' || character == '\'' => {
                quote = Some(character);
                in_token = true;
            }
            None if character.is_whitespace() => {
                if in_token {
                    args.push(std::mem::take(&mut current));
                    in_token = false;
                }
            }
            None => {
                current.push(character);
                in_token = true;
            }
        }
    }

    if in_token {
        args.push(current);
    }

    args
}

fn spawn_program(
    program: &Path,
    arguments: &[String],
    work_dir: Option<&Path>,
) -> std::io::Result<Child> {
    #[cfg(windows)]
    let mut command = {
        let is_command_script = program
            .extension()
            .and_then(|extension| extension.to_str())
            .is_some_and(|extension| {
                extension.eq_ignore_ascii_case("bat") || extension.eq_ignore_ascii_case("cmd")
            });
        if is_command_script {
            let shell = env::var_os("COMSPEC").unwrap_or_else(|| "cmd.exe".into());
            let mut command = ProcessCommand::new(shell);
            command.args(["/D", "/S", "/C"]).arg(program);
            command
        } else {
            ProcessCommand::new(program)
        }
    };

    #[cfg(not(windows))]
    let mut command = ProcessCommand::new(program);

    command.args(arguments);
    if let Some(directory) = work_dir {
        command.current_dir(directory);
    }
    command.spawn()
}

fn wait_for_children(children: Vec<(String, Child)>) -> Result<(), String> {
    let mut failures = Vec::new();

    for (name, mut child) in children {
        match child.wait() {
            Ok(status) if status.success() => println!("exited {name} ({status})"),
            Ok(status) => failures.push(format!("{name} exited with {status}")),
            Err(error) => failures.push(format!("cannot wait for {name}: {error}")),
        }
    }

    if failures.is_empty() {
        Ok(())
    } else {
        Err(failures.join("; "))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_config() -> Config {
        toml::from_str(
            r#"
                [[item]]
                name = "Visual Studio Code"
                alias = ["code", "vsc"]
                keywords = ["editor", "development"]
                path = ["tools/code.exe"]
                start_args = ["--reuse-window"]
                work_dir = "work"
                description = "Source code editor"

                [[item]]
                name = "Terminal"
                alias = ["term"]
                path = ["cmd.exe"]
            "#,
        )
        .expect("sample config should parse")
    }

    #[test]
    fn parses_defaults_for_optional_item_fields() {
        let config: Config = toml::from_str(
            r#"
                [[item]]
                name = "Example"
            "#,
        )
        .expect("minimal item should parse");

        assert_eq!(config.item.len(), 1);
        assert!(config.item[0].alias.is_empty());
        assert!(config.item[0].path.is_empty());
        assert!(config.item[0].work_dir.as_os_str().is_empty());
    }

    #[test]
    fn finds_names_and_aliases_case_insensitively() {
        let config = sample_config();

        assert_eq!(
            find_item(&config.item, "visual studio CODE").map(|item| item.name.as_str()),
            Some("Visual Studio Code")
        );
        assert_eq!(
            find_item(&config.item, "VSC").map(|item| item.name.as_str()),
            Some("Visual Studio Code")
        );
    }

    #[test]
    fn searches_all_supplied_terms_across_fields() {
        let config = sample_config();
        let found = search_items(&config.item, &["editor", "code.exe"]);

        assert_eq!(found.len(), 1);
        assert_eq!(found[0].name, "Visual Studio Code");
        assert!(search_items(&config.item, &["editor", "terminal"]).is_empty());
    }

    #[test]
    fn rejects_duplicate_names_and_aliases() {
        let config: Config = toml::from_str(
            r#"
                [[item]]
                name = "First"
                alias = ["shared"]

                [[item]]
                name = "SHARED"
            "#,
        )
        .expect("sample config should parse");

        assert!(validate_config(&config).is_err());
    }

    #[test]
    fn resolves_relative_program_paths_from_config_directory() {
        assert_eq!(
            resolve_program(Path::new("C:/config"), Path::new("tools/app.exe")),
            PathBuf::from("C:/config/tools/app.exe")
        );
        assert_eq!(
            resolve_program(Path::new("C:/config"), Path::new("app.exe")),
            PathBuf::from("app.exe")
        );
    }

    #[test]
    fn work_dir_defaults_to_the_program_directory() {
        // An explicit work_dir always wins.
        assert_eq!(
            resolve_work_dir(
                Some(PathBuf::from("C:/explicit")),
                Path::new("C:/apps/bin/app.exe")
            ),
            Some(PathBuf::from("C:/explicit"))
        );

        // With nothing configured, the program's own directory is used, so that
        // programs resolving their resources relative to themselves still start.
        assert_eq!(
            resolve_work_dir(None, Path::new("C:/apps/bin/app.exe")),
            Some(PathBuf::from("C:/apps/bin"))
        );

        // A bare name (PATH lookup) has no directory to offer: inherit as before.
        assert_eq!(resolve_work_dir(None, Path::new("app.exe")), None);
    }

    #[test]
    fn splits_extra_arguments_on_whitespace() {
        assert_eq!(split_args("-m 512"), vec!["-m", "512"]);
        assert_eq!(
            split_args("   -drive   format=raw,file=bios.img   "),
            vec!["-drive", "format=raw,file=bios.img"]
        );
        assert_eq!(split_args("-s -S"), vec!["-s", "-S"]);
        assert!(split_args("").is_empty());
        assert!(split_args("   ").is_empty());
    }

    #[test]
    fn keeps_quoted_arguments_and_windows_paths_intact() {
        // A quoted value keeps its spaces but loses the quotes.
        assert_eq!(
            split_args(r#"-drive "format=raw,file=D:/my os/bios.img""#),
            vec!["-drive", "format=raw,file=D:/my os/bios.img"]
        );
        // Backslashes are data, not escapes.
        assert_eq!(
            split_args(r"-drive format=raw,file=D:\XiangMu\QH_OS\bios.img"),
            vec!["-drive", r"format=raw,file=D:\XiangMu\QH_OS\bios.img"]
        );
        // An explicitly quoted empty value is still an argument.
        assert_eq!(split_args(r#"-name "" "#), vec!["-name", ""]);
        // Single quotes work the same way.
        assert_eq!(split_args("--title 'my kernel'"), vec!["--title", "my kernel"]);
    }
}

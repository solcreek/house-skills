// Conventions: ../audit/references/conventions.md in the cli plugin.
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use anyhow::{anyhow, bail, Context};
use clap::error::ErrorKind;
use clap::{CommandFactory, Parser, Subcommand};
use serde_json::{json, Value};

/// One line on what it is for.
///
/// Exit codes: 0 ok, 1 failed, 2 usage.
#[derive(Parser)]
#[command(name = "tool", version, after_help = "Run 'tool describe' for every command as JSON.")]
struct Cli {
    /// Print one JSON document on stdout
    #[arg(long, global = true)]
    json: bool,
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Print a greeting
    #[command(after_help = "Examples:\n  tool greet Ada\n  tool greet Ada --json")]
    Greet {
        /// Who to greet
        name: String,
    },
    /// Create an empty file under the current directory (changes things)
    #[command(after_help = "Examples:\n  tool touch notes.txt --dry-run\n  tool touch notes.txt --yes --json")]
    Touch {
        /// The file to create, inside the current directory
        path: PathBuf,
        #[command(flatten)]
        change: Change,
    },
    /// Print every command, flag and exit code as JSON
    Describe,
}

/// Every mutating command flattens this: clap then requires exactly one of
/// the two, and answers neither or both with a usage error.
#[derive(clap::Args)]
#[group(required = true, multiple = false)]
struct Change {
    /// Show what would change; change nothing
    #[arg(long)]
    dry_run: bool,
    /// Apply the change
    #[arg(long)]
    yes: bool,
}

// Every exit code, in help and in `describe`. Add to it; never reuse one.
const FAILED: u8 = 1;
const USAGE: u8 = 2;

fn main() -> ExitCode {
    let json_asked = std::env::args().any(|a| a == "--json");
    let cli = match Cli::try_parse() {
        Ok(cli) => cli,
        // --help and --version: clap prints them to stdout and exits 0.
        Err(e) if matches!(e.kind(), ErrorKind::DisplayHelp | ErrorKind::DisplayVersion) => e.exit(),
        Err(e) => {
            let _ = e.print();
            // Under --json a usage error is JSON too (rubric A5). Parsing
            // failed, so look at the arguments themselves.
            if json_asked {
                let first = e.to_string().lines().next().unwrap_or_default().trim_start_matches("error: ").to_string();
                println!("{}", json!({ "error": first, "usage": true }));
            }
            return ExitCode::from(USAGE);
        }
    };
    match run(&cli) {
        Ok(code) => code,
        Err(e) => {
            eprintln!("tool: {e:#}");
            if cli.json {
                println!("{}", json!({ "error": format!("{e:#}"), "usage": false }));
            }
            ExitCode::from(FAILED)
        }
    }
}

fn run(cli: &Cli) -> anyhow::Result<ExitCode> {
    match &cli.command {
        Command::Greet { name } => {
            let greeting = format!("hello, {}", plain("NAME", name)?);
            print(cli, json!({ "greeting": greeting }), &greeting);
        }
        Command::Touch { path, change } => {
            let raw = path.to_string_lossy();
            plain("PATH", &raw)?;
            let target = inside(&std::env::current_dir()?, path)?;
            // Already there is success with nothing changed (rubric S5).
            let changed = !target.exists();
            if changed && change.yes {
                std::fs::OpenOptions::new().write(true).create_new(true).open(&target)?;
            }
            let verb = if !changed { "exists" } else if change.dry_run { "would create" } else { "created" };
            let shown = target.display().to_string();
            print(cli, json!({ "path": shown, "changed": changed, "dryRun": change.dry_run }), &format!("{verb} {shown}"));
        }
        Command::Describe => println!("{}", serde_json::to_string_pretty(&describe_root())?),
    }
    Ok(ExitCode::SUCCESS)
}

fn print(cli: &Cli, json: Value, text: &str) {
    if cli.json {
        println!("{json}");
    } else {
        println!("{text}");
    }
}

// Control characters in an argument are refused (rubric A9).
fn plain<'a>(what: &str, s: &'a str) -> anyhow::Result<&'a str> {
    if s.chars().any(|c| c.is_control()) {
        bail!("{what} contains a control character");
    }
    Ok(s)
}

// An output path must land inside root once symlinks are resolved (rubric A9).
fn inside(root: &Path, path: &Path) -> anyhow::Result<PathBuf> {
    let real_root = root.canonicalize()?;
    let parent = root.join(path.parent().unwrap_or(Path::new("")));
    let parent = parent.canonicalize().with_context(|| format!("no such directory: {}", parent.display()))?;
    let name = path.file_name().ok_or_else(|| anyhow!("{} names no file", path.display()))?;
    let target = parent.join(name);
    if !target.starts_with(&real_root) || target == real_root {
        bail!("{} is outside {}; tool writes only under the current directory", path.display(), real_root.display());
    }
    Ok(target)
}

// The same definitions as --help, as data (rubric A10), in the shape every
// template's describe prints: see references/describe.md.
fn describe_root() -> Value {
    // build() adds --help and --version and copies global flags down, so
    // describe lists what --help shows.
    let mut cmd = Cli::command();
    cmd.build();
    let mut d = describe(&cmd);
    d["version"] = json!(cmd.get_version());
    d["exitCodes"] = json!({ "ok": 0, "failed": FAILED, "usage": USAGE });
    d
}

fn describe(cmd: &clap::Command) -> Value {
    let flags: Vec<Value> = cmd
        .get_arguments()
        .filter(|a| !a.is_hide_set() && !a.is_positional())
        .map(|a| {
            let default = a.get_default_values().first().map(|v| v.to_string_lossy().to_string());
            json!({
                "name": a.get_long().unwrap_or(a.get_id().as_str()),
                "short": a.get_short().map(|c| c.to_string()),
                "takesValue": a.get_action().takes_values(),
                "default": default.filter(|_| a.get_action().takes_values()),
                "description": a.get_help().map(|s| s.to_string()).unwrap_or_default(),
            })
        })
        .collect();
    let usage = cmd.clone().render_usage().to_string();
    let mut d = json!({
        "name": cmd.get_name(),
        "summary": cmd.get_about().map(|s| s.to_string()).unwrap_or_default(),
        "usage": usage.trim_start_matches("Usage: "),
        "flags": flags,
        "commands": cmd.get_subcommands().filter(|c| c.get_name() != "help").map(describe).collect::<Vec<_>>(),
    });
    // A command that flattens Change mutates.
    d["mutates"] = json!(cmd.get_arguments().any(|a| a.get_long() == Some("yes")));
    d
}

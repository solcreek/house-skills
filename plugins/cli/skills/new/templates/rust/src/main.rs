// Conventions: ../audit/references/conventions.md in the cli plugin.
use std::path::{Component, Path, PathBuf};
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
    /// Create an empty file under the current directory
    #[command(after_help = "Changes things: pass --dry-run to preview, --yes to apply.\n\nExamples:\n  tool touch notes.txt --dry-run\n  tool touch notes.txt --yes --json")]
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

/// A mistake in the invocation that clap cannot see (an empty or malformed
/// value): exit 2, like clap's own usage errors.
#[derive(Debug)]
struct Usage(String);

impl std::fmt::Display for Usage {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for Usage {}

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
            let usage = e.is::<Usage>();
            eprintln!("tool: {e:#}");
            if usage {
                eprintln!("Run 'tool --help' for usage.");
            }
            if cli.json {
                println!("{}", json!({ "error": format!("{e:#}"), "usage": usage }));
            }
            ExitCode::from(if usage { USAGE } else { FAILED })
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
                std::fs::OpenOptions::new()
                    .write(true)
                    .create_new(true)
                    .open(&target)
                    .with_context(|| format!("cannot create {}", target.display()))?;
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

// An empty argument or a control character in one is a usage error (rubric A9).
fn plain<'a>(what: &str, s: &'a str) -> anyhow::Result<&'a str> {
    if s.is_empty() {
        return Err(Usage(format!("{what} is empty")).into());
    }
    if s.chars().any(|c| c.is_control()) {
        return Err(Usage(format!("{what} contains a control character")).into());
    }
    Ok(s)
}

// The file an output path names, refused unless it is plainly inside root
// (rubric A9). Every template's inside() refuses the same things: a `..`
// part, a trailing slash, the root itself, a parent that resolves outside
// root, and a last part that is a symlink (dangling or not), so the write
// can never follow one.
fn inside(root: &Path, path: &Path) -> anyhow::Result<PathBuf> {
    let shown = path.display();
    if path.components().any(|c| c == Component::ParentDir) {
        bail!("{shown}: '..' is not allowed in an output path");
    }
    let raw = path.to_string_lossy();
    if raw.ends_with('/') || raw.ends_with(std::path::MAIN_SEPARATOR) {
        bail!("{shown}: names a directory, not a file");
    }
    let real_root = root.canonicalize()?;
    let full = root.join(path);
    let name = full.file_name().ok_or_else(|| anyhow!("{shown}: is {} itself", real_root.display()))?;
    let parent = full.parent().unwrap_or(Path::new("/"));
    let parent = parent.canonicalize().with_context(|| format!("{shown}: no such directory: {}", parent.display()))?;
    let target = parent.join(name);
    if target == real_root {
        bail!("{shown}: is {} itself", real_root.display());
    }
    if !target.starts_with(&real_root) {
        bail!("{shown}: outside {}; tool writes only under the current directory", real_root.display());
    }
    if target.symlink_metadata().is_ok_and(|m| m.file_type().is_symlink()) {
        bail!("{shown}: is a symlink; refusing to write through it");
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
    let root = d.as_object_mut().unwrap();
    // The top level has no usage line or mutates of its own (describe.md).
    root.remove("usage");
    root.remove("mutates");
    root.insert("version".into(), json!(cmd.get_version()));
    root.insert("exitCodes".into(), json!({ "ok": 0, "failed": FAILED, "usage": USAGE }));
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

// Conventions: ../audit/references/conventions.md in the cli plugin.
use std::path::{Component, Path, PathBuf};
use std::process::ExitCode;

use anyhow::{anyhow, bail, Context};
use clap::error::ErrorKind;
use clap::{CommandFactory, FromArgMatches, Parser, Subcommand};
use serde_json::{json, Value};

/// One line on what it is for.
#[derive(Parser)]
#[command(name = "tool", version)]
struct Cli {
    /// Print one JSON document on stdout
    #[arg(long, global = true)]
    json: bool,
    /// None: no command given, so print help (to stdout, exit 0).
    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Subcommand)]
enum Command {
    /// List the files under a directory, largest first
    Files {
        /// The directory to list
        #[arg(default_value = ".")]
        dir: PathBuf,
        #[command(flatten)]
        limit: Limit,
    },
    /// Create an empty file under the current directory
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

/// Every list command flattens this. The value name N marks a whole number:
/// describe reports the flag's type as "integer".
#[derive(clap::Args)]
struct Limit {
    /// Show at most N items; 0 for all
    #[arg(long, value_name = "N", default_value = "20", value_parser = whole_number)]
    limit: usize,
}

// Base-10 digits only, as in every template: no sign, no 0x, no underscore.
fn whole_number(s: &str) -> Result<usize, String> {
    if s.is_empty() || !s.bytes().all(|b| b.is_ascii_digit()) {
        return Err("takes a whole number".into());
    }
    s.parse().map_err(|_| "is too large".into())
}

// Every exit code, in help and in `describe`. Add to it; never reuse one.
const EXIT: &[(&str, u8)] = &[("ok", 0), ("failed", 1), ("usage", 2), ("partial", 3)];
const FAILED: u8 = 1;
const USAGE: u8 = 2;
const PARTIAL: u8 = 3;
const EXIT_HELP: &str = "Exit codes: 0 ok, 1 failed, 2 usage, 3 partial";

// Each command's examples: shown in its help and listed by describe.
const EXAMPLES: &[(&str, &[&str])] = &[
    ("files", &["tool files", "tool files src --limit 5 --json"]),
    ("touch", &["tool touch notes.txt --dry-run", "tool touch notes.txt --yes --json"]),
    ("describe", &["tool describe"]),
];

/// The clap command with what derive cannot add: each command's examples and
/// the exit codes on every help page (rubric E4).
fn command() -> clap::Command {
    let mut cmd = Cli::command().after_help(format!("Run 'tool describe' for every command as JSON.\n\n{EXIT_HELP}"));
    for (name, examples) in EXAMPLES {
        let mutating = cmd.find_subcommand(name).is_some_and(mutates);
        let note = if mutating { "Changes things: pass --dry-run to preview, --yes to apply.\n\n" } else { "" };
        let text = format!("{note}Examples:\n  {}\n\n{EXIT_HELP}", examples.join("\n  "));
        cmd = cmd.mut_subcommand(name, |c| c.after_help(text));
    }
    cmd
}

fn mutates(cmd: &clap::Command) -> bool {
    cmd.get_arguments().any(|a| a.get_long() == Some("yes"))
}

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
    // Error path only: parsing may fail before it sees --json.
    let json_asked = std::env::args().skip(1).take_while(|a| a != "--").any(|a| a == "--json");
    let parsed = command().try_get_matches().and_then(|m| Cli::from_arg_matches(&m));
    let cli = match parsed {
        Ok(cli) => cli,
        // --help and --version: clap prints them to stdout and exits 0.
        Err(e) if matches!(e.kind(), ErrorKind::DisplayHelp | ErrorKind::DisplayVersion) => e.exit(),
        Err(e) => {
            let _ = e.print();
            // Under --json a usage error is JSON too (rubric A5): the message
            // up to its first blank line, which names the flag.
            if json_asked {
                let text = e.to_string();
                let first: Vec<&str> = text.lines().take_while(|l| !l.trim().is_empty()).map(str::trim).collect();
                let message = first.join(" ").trim_start_matches("error: ").to_string();
                println!("{}", json!({ "error": message, "usage": true }));
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
    let Some(cmd) = &cli.command else {
        print!("{}", command().render_help());
        return Ok(ExitCode::SUCCESS);
    };
    match cmd {
        Command::Files { dir, limit } => {
            plain("DIR", &dir.to_string_lossy())?;
            // Lexically, like the other templates: `a/..` is the current directory.
            let given = lexical(&std::path::absolute(dir)?);
            match given.metadata() {
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => bail!("no such directory: {}", given.display()),
                Err(e) => bail!("cannot read {}: {}", given.display(), reason(&e)),
                Ok(m) if !m.is_dir() => bail!("not a directory: {}", given.display()),
                Ok(_) => {}
            }
            // Walk the real path, so a symlinked DIR (or current directory)
            // lists its files, and paths compare with the tool's own homes.
            let root = given.canonicalize()?;
            // What cannot be read inside the tree is a problem, never silently
            // "nothing there" (rubric A7). The root itself unreadable is a failure.
            if let Err(e) = std::fs::read_dir(&root) {
                bail!("cannot read {}: {}", given.display(), reason(&e));
            }
            let mut found: Vec<(String, u64)> = Vec::new();
            let mut problems: Vec<Value> = Vec::new();
            walk(&root, &root, &homes(), &mut found, &mut problems);
            found.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
            let list = capped(&found, limit.limit);
            if cli.json {
                let items: Vec<Value> = list.items.iter().map(|(p, b)| json!({ "path": p, "bytes": b })).collect();
                let mut doc = json!({
                    "root": root, "items": items, "total": list.total, "shown": list.shown, "truncated": list.truncated,
                });
                if !problems.is_empty() {
                    doc["problems"] = json!(problems);
                }
                println!("{doc}");
            } else {
                for (p, b) in list.items {
                    println!("{b}\t{}", printable(p));
                }
            }
            for p in &problems {
                let (input, error) = (p["input"].as_str().unwrap_or(""), p["error"].as_str().unwrap_or(""));
                eprintln!("tool: cannot read {}: {error}", printable(input));
            }
            return Ok(ExitCode::from(if problems.is_empty() { 0 } else { PARTIAL }));
        }
        Command::Touch { path, change } => {
            plain("PATH", &path.to_string_lossy())?;
            let target = inside(&std::env::current_dir()?, path)?;
            // Already there is success with nothing changed (rubric S5).
            let changed = !target.exists();
            if changed && change.yes {
                std::fs::OpenOptions::new()
                    .write(true)
                    .create_new(true)
                    .open(&target)
                    .map_err(|e| anyhow!("cannot create {}: {}", target.display(), reason(&e)))?;
            }
            let shown = target.display().to_string();
            // State: what this tool created, kept across runs.
            let recorded = changed && change.yes && append_state("history.jsonl", &json!({ "path": shown }));
            let verb = if !changed { "exists" } else if change.dry_run { "would create" } else { "created" };
            if cli.json {
                println!("{}", json!({ "path": shown, "changed": changed, "dryRun": change.dry_run, "recorded": recorded }));
            } else {
                println!("{verb} {shown}");
            }
        }
        Command::Describe => println!("{}", serde_json::to_string_pretty(&describe_root())?),
    }
    Ok(ExitCode::SUCCESS)
}

fn walk(root: &Path, dir: &Path, own: &[PathBuf], found: &mut Vec<(String, u64)>, problems: &mut Vec<Value>) {
    let rel = |p: &Path| p.strip_prefix(root).unwrap_or(p).display().to_string();
    let entries = match std::fs::read_dir(dir) {
        Ok(e) => e,
        Err(e) => return problems.push(json!({ "input": rel(dir), "error": reason(&e) })),
    };
    for entry in entries {
        // An entry the listing could not read leaves it incomplete: a problem.
        let entry = match entry {
            Ok(e) => e,
            Err(e) => {
                problems.push(json!({ "input": rel(dir), "error": reason(&e) }));
                continue;
            }
        };
        let path = entry.path();
        // DirEntry::file_type does not follow symlinks.
        match entry.file_type() {
            Ok(t) if t.is_dir() => {
                if !own.contains(&path) {
                    walk(root, &path, own, found, problems)
                }
            }
            Ok(t) if t.is_file() => match entry.metadata() {
                Ok(m) => found.push((rel(&path), m.len())),
                Err(e) => problems.push(json!({ "input": rel(&path), "error": reason(&e) })),
            },
            Ok(_) => {}
            Err(e) => problems.push(json!({ "input": rel(&path), "error": reason(&e) })),
        }
    }
}

// An error's reason without the path it repeats: "permission denied", the
// same words in every template.
fn reason(e: &std::io::Error) -> String {
    e.kind().to_string()
}

// A path with `.` and `..` resolved by its text, as Node's resolve and Go's
// Clean do.
fn lexical(p: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for c in p.components() {
        match c {
            Component::ParentDir => {
                out.pop();
            }
            Component::CurDir => {}
            c => out.push(c),
        }
    }
    out
}

struct Capped<'a, T> {
    items: &'a [T],
    total: usize,
    shown: usize,
    truncated: bool,
}

// A list, capped (conventions: JSON output, Human output). The note goes to
// stderr so a pipe sees only results.
fn capped<T>(all: &[T], limit: usize) -> Capped<'_, T> {
    let items = if limit == 0 { all } else { &all[..all.len().min(limit)] };
    if items.len() < all.len() {
        eprintln!("{} of {} shown; --limit 0 for all", items.len(), all.len());
    }
    Capped { items, total: all.len(), shown: items.len(), truncated: items.len() < all.len() }
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

// Text from files or the network, made safe to print to a terminal.
fn printable(s: &str) -> String {
    s.chars().map(|c| if c.is_control() && c != '\t' { '?' } else { c }).collect()
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
        bail!("{shown}: outside {}", real_root.display());
    }
    if target.symlink_metadata().is_ok_and(|m| m.file_type().is_symlink()) {
        bail!("{shown}: is a symlink; refusing to write through it");
    }
    Ok(target)
}

// Where config, cache and state live (conventions: State). TOOL_HOME moves
// all three under one directory; tests set it. None when there is no home
// to put them in (HOME unset, as under cron): never the current directory.
fn home(kind: &str) -> Option<PathBuf> {
    if let Some(dir) = std::env::var_os("TOOL_HOME").filter(|d| !d.is_empty()) {
        return std::path::absolute(PathBuf::from(dir)).ok().map(|d| d.join(kind));
    }
    let (var, fallback) = match kind {
        "config" => ("XDG_CONFIG_HOME", ".config"),
        "cache" => ("XDG_CACHE_HOME", ".cache"),
        _ => ("XDG_STATE_HOME", ".local/state"),
    };
    // XDG says to ignore a relative path.
    let base = std::env::var_os(var).map(PathBuf::from).filter(|p| p.is_absolute());
    let base = base.or_else(|| std::env::var_os("HOME").map(PathBuf::from).filter(|p| p.is_absolute()).map(|h| h.join(fallback)))?;
    Some(base.join("tool"))
}

// The tool's own directories that exist, by real path: a walk skips them
// (conventions: Paths), so a TOOL_HOME inside the tree is not listed.
fn homes() -> Vec<PathBuf> {
    let mut dirs: Vec<PathBuf> = ["config", "cache", "state"].iter().filter_map(|k| home(k)).collect();
    if let Some(d) = std::env::var_os("TOOL_HOME").filter(|d| !d.is_empty()) {
        dirs.push(PathBuf::from(d));
    }
    dirs.iter().filter_map(|d| d.canonicalize().ok()).collect()
}

// Append one JSON line to a state file (conventions: State). Appending never
// reads the file back, so a corrupt or unreadable one is never overwritten,
// and concurrent runs never lose each other's lines. Recording is not the
// command's action: a failure is a warning, and the result says
// recorded: false.
fn append_state(name: &str, entry: &Value) -> bool {
    use std::io::Write;
    use std::os::unix::fs::{DirBuilderExt, OpenOptionsExt};
    let Some(state) = home("state") else {
        eprintln!("tool: warning: not recorded: no home directory; set TOOL_HOME or XDG_STATE_HOME");
        return false;
    };
    let result = std::fs::DirBuilder::new()
        .recursive(true)
        .mode(0o700)
        .create(&state)
        .map_err(|e| reason(&e))
        .and_then(|_| inside(&state, Path::new(name)).map_err(|e| format!("{e:#}")))
        .and_then(|file| {
            let mut f = std::fs::OpenOptions::new().append(true).create(true).mode(0o600).open(file).map_err(|e| reason(&e))?;
            f.write_all(format!("{entry}\n").as_bytes()).map_err(|e| reason(&e))
        });
    if let Err(why) = &result {
        eprintln!("tool: warning: not recorded in {}: {why}", state.join(name).display());
    }
    result.is_ok()
}

// The same definitions as --help, as data (rubric A10), in the shape every
// template's describe prints: see references/describe.md.
fn describe_root() -> Value {
    // build() adds --help and --version and copies global flags down, so
    // describe lists what --help shows.
    let mut cmd = command();
    cmd.build();
    let codes: serde_json::Map<String, Value> = EXIT.iter().map(|(k, v)| (k.to_string(), json!(v))).collect();
    let commands: Vec<Value> = cmd.get_subcommands().filter(|c| c.get_name() != "help").map(describe).collect();
    json!({
        "name": cmd.get_name(),
        "version": cmd.get_version(),
        "summary": cmd.get_about().map(|s| s.to_string()).unwrap_or_default(),
        "exitCodes": codes,
        "flags": flags(&cmd),
        "commands": commands,
    })
}

fn describe(cmd: &clap::Command) -> Value {
    let args: Vec<Value> = cmd
        .get_positionals()
        .map(|a| {
            let name = a.get_value_names().and_then(|n| n.first()).map(|n| n.to_string());
            json!({
                "name": name.unwrap_or_else(|| a.get_id().to_string().to_uppercase()),
                "required": a.is_required_set(),
                "default": a.get_default_values().first().map(|v| v.to_string_lossy().to_string()),
            })
        })
        .collect();
    let examples = EXAMPLES.iter().find(|(n, _)| *n == cmd.get_name()).map(|(_, e)| e.to_vec()).unwrap_or_default();
    let usage = cmd.clone().render_usage().to_string();
    json!({
        "name": cmd.get_name(),
        "summary": cmd.get_about().map(|s| s.to_string()).unwrap_or_default(),
        "usage": usage.trim_start_matches("Usage: "),
        "mutates": mutates(cmd),
        "args": args,
        "flags": flags(cmd),
        "examples": examples,
        "commands": cmd.get_subcommands().filter(|c| c.get_name() != "help").map(describe).collect::<Vec<_>>(),
    })
}

fn flags(cmd: &clap::Command) -> Vec<Value> {
    cmd.get_arguments()
        .filter(|a| !a.is_hide_set() && !a.is_positional())
        .map(|a| {
            let takes = a.get_action().takes_values();
            let whole = a.get_value_names().is_some_and(|v| v.iter().any(|n| n.as_str() == "N"));
            json!({
                "name": a.get_long().unwrap_or(a.get_id().as_str()),
                "short": a.get_short().map(|c| c.to_string()),
                "type": if !takes { "boolean" } else if whole { "integer" } else { "string" },
                "default": a.get_default_values().first().filter(|_| takes).map(|v| v.to_string_lossy().to_string()),
                "description": a.get_help().map(|s| s.to_string()).unwrap_or_default(),
            })
        })
        .collect()
}

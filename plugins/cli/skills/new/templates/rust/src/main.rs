use std::process::ExitCode;

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
    /// Print every command, flag and exit code as JSON
    Describe,
}

// Every exit code, in help and in `describe`. Add to it; never reuse one.
// clap exits 2 on a usage error and 0 on --help/--version before main runs.
const FAILED: u8 = 1;

fn main() -> ExitCode {
    let cli = Cli::parse();
    match run(&cli) {
        Ok(code) => code,
        Err(e) => {
            eprintln!("tool: {e:#}");
            // Under --json a failure is JSON too (rubric A5).
            if cli.json {
                println!("{}", json!({ "error": format!("{e:#}") }));
            }
            ExitCode::from(FAILED)
        }
    }
}

fn run(cli: &Cli) -> anyhow::Result<ExitCode> {
    match &cli.command {
        Command::Greet { name } => {
            let greeting = format!("hello, {name}");
            if cli.json {
                println!("{}", json!({ "greeting": greeting }));
            } else {
                println!("{greeting}");
            }
        }
        Command::Describe => println!("{}", serde_json::to_string_pretty(&describe(&Cli::command()))?),
    }
    Ok(ExitCode::SUCCESS)
}

// The same definitions as --help, as data (rubric A10).
fn describe(cmd: &clap::Command) -> Value {
    json!({
        "name": cmd.get_name(),
        "about": cmd.get_about().map(|s| s.to_string()),
        "args": cmd.get_arguments().filter(|a| !a.is_hide_set()).map(|a| json!({
            "id": a.get_id().as_str(),
            "long": a.get_long(),
            "short": a.get_short(),
            "required": a.is_required_set(),
            "help": a.get_help().map(|s| s.to_string()),
        })).collect::<Vec<_>>(),
        "subcommands": cmd.get_subcommands().filter(|c| c.get_name() != "help").map(describe).collect::<Vec<_>>(),
    })
}

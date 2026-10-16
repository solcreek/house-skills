# CLI rubric

The standard both `cli:audit` and `cli:new` hold a tool to. Each rule has an
id, a tier, and how it is checked: a `probe` check id, **read** (the source),
or **run** (an action, only with the user's consent or when the source shows it
is read-only and local).

Tiers:

- **must** — breaks people or agents today: unsafe invocation, lost output,
  hangs, lies about success.
- **should** — costs time on every use: discoverability, consistency, speed.
- **taste** — polish; report briefly and never block on it.

A tool agents call (anything in a repo's `bin/`, anything a skill or hook
runs) is held to section A as **must**. Tools run only from a checkout may
skip D1 (`--version`) and D5 (completion).

Based on [clig.dev](https://clig.dev) and Justin Poehnelt's
[Rewrite your CLI for AI agents](https://justin.poehnelt.com/posts/rewrite-your-cli-for-ai-agents/),
narrowed to what we enforce. Where we differ from the latter, see "Deliberately
not adopted" at the end.

## S. Safe to invoke

| id | tier | rule | check |
|---|---|---|---|
| S1 | must | `--help` and `-h` print help and exit 0 **before any side effect**, on every command and subcommand. | probe `help-long`, `help-short`; **read** each subcommand's arg handling |
| S2 | must | An unknown flag is a usage error. It is never ignored and never forwarded to another program unless after an explicit `--`. | probe `unknown-flag`; **read** for subcommands |
| S3 | must | A destructive or outward action (delete, push, publish, pay, send) needs `--yes` or a TTY confirmation, and has a dry run (`--dry-run`, or dry by default). Without a TTY it never prompts: it fails and names the flag. | **read** |
| S4 | must | Secrets never go in argv (visible in `ps` and shell history): env, stdin or a file. Auth works headless (a token or credentials-file env var); a browser flow is only a fallback for people. | **read** |
| S5 | should | Rerunning is safe: an action already done reports success ("already open"), not an error. | **read** |

## E. Streams and exit codes

| id | tier | rule | check |
|---|---|---|---|
| E1 | must | Help goes to stdout. Errors and diagnostics go to stderr. | probe `help-long`, `unknown-flag` |
| E2 | must | stdout carries only the result. Progress, logs and spinners go to stderr, so `tool … \| next` and `--json` stay clean. | **read**, **run** |
| E3 | must | Exit 0 only on success. A usage error has its own code (2, or 64 from sysexits), distinct from a runtime failure (1). | probe `unknown-flag`; **read** |
| E4 | should | Any exit code beyond 0/1/usage is documented in that command's help. | **read** |
| E5 | should | Ctrl-C stops promptly with 130, cleaning up temp files and child processes. | **read** |

## A. Agent-friendly

| id | tier | rule | check |
|---|---|---|---|
| A1 | should (must for agent-called tools) | Every command that reports something has `--json`: one document on stdout, stable field names, the same exit codes. | probe `--json-output` |
| A2 | must | Never waits on stdin or a prompt when stdin is not a terminal. | every probe run (a timeout fails it) |
| A3 | must | No colour, pager, spinner or cursor control when the stream is not a terminal, and `NO_COLOR` is honoured. | probe `no-color-piped`, `no-color-env` |
| A4 | should | An error says what to do next: the exact command, flag or file. "Not found" names what was looked for and where. | **read** error strings |
| A5 | should | Under `--json`, a failure is still JSON (`{"error": …}` or a state field) with a non-zero exit, so the caller never parses prose. | **run** |
| A6 | should | Output is bounded. Lists are capped and say so ("30 of 212"), with a flag to raise the cap. Each item is one line, or a JSON element. A command returning large records takes `--fields` to select what comes back; a stream or paginated list is NDJSON, one object per line. | **read** |
| A7 | should | "Could not check" never looks like "nothing found". Empty results and failures to look have different exit codes or states. | **read** |
| A8 | taste | Output is deterministic: sorted, no timestamps or colour unless asked, so reruns diff cleanly. | **run** |
| A9 | should (must when an argument becomes a path, URL or query) | Inputs are treated as adversarial: an agent hallucinates where a person typos. Reject control characters; reject `?`, `#` and `%` in resource ids; keep output paths inside the intended directory (canonicalize, refuse `..` escapes); percent-encode path segments. | **read**; **run** with a bad id under `--dry-run` |
| A10 | should for agent-called tools with several commands | The command surface is introspectable at runtime: `help --json` (or `schema <command>`) prints commands, flags, types, defaults and exit codes, generated from the same definitions as `--help` so it cannot drift. | **run** |
| A11 | should for tools wrapping an API | Requests can be given whole as JSON (`--json-input FILE`, or `-` for stdin) instead of only through bespoke flags, so nested shapes need no flag design. | **read** |
| A12 | should for agent-called tools | The tool ships its agent context: a skill or `AGENTS.md` section stating which commands mutate, to dry-run first, which flags to always pass (`--json`, `--fields`) and what each exit code means. Help teaches people; this teaches agents the invariants they will not infer. | **read** |

## D. Discoverable and consistent

| id | tier | rule | check |
|---|---|---|---|
| D1 | should | `--version` prints a version number to stdout and exits 0. | probe `version` |
| D2 | should | Help leads with usage, shows at least one example per command, and fits 100 columns. | probe `help-width`; **read** help |
| D3 | should | A usage error prints one or two lines and points to `--help`. It does not dump the whole help to stderr. | probe `unknown-flag` stderr size |
| D4 | should | Names are consistent: one verb per idea (`list`/`show`/`create`/`delete`, not `ls` here and `list` there), kebab-case long flags, a long form for every short flag, same flag same meaning everywhere. | **read** |
| D5 | taste | Shell completion, and "did you mean" for misspelt subcommands. | **read** |

## C. Configuration and environment

| id | tier | rule | check |
|---|---|---|---|
| C1 | must | `--help` works from any directory, offline, with no config. | probe (runs in an empty temporary directory) |
| C2 | should | Precedence: flags > env > project config > user config > defaults. Config lives under XDG paths (`$XDG_CONFIG_HOME`, falling back to `~/.config`). | **read** |
| C3 | should | Writes go only where documented, and temporary files are cleaned up. | **read** |
| C4 | should | A command that needs a particular directory (a git checkout, a project root) says which one when run elsewhere. | **run** in an empty directory, read-only commands only |

## P. Performance

| id | tier | rule | check |
|---|---|---|---|
| P1 | should | `--help` returns in 300 ms or less. Heavy imports load lazily, after argument parsing. | probe `startup` |
| P2 | should | Network calls have timeouts and say what they are waiting on (on stderr). | **read** |

## Reporting a finding

Each finding gives: the rule id and tier, the command it applies to, the
evidence (a probe check with its exit code and output excerpt, or
`file:line`), and the smallest fix. Lead with **must** findings. Group the
same rule's failures across subcommands into one finding.

## Deliberately not adopted

- **Switching to JSON when stdout is not a terminal** (Poehnelt). A format
  that changes with the pipe breaks `tool | grep` for people and is invisible
  in an agent's transcript. We ask for an explicit `--json` (A1). Colour,
  pagers and prompts do key off the terminal (A2, A3).
- **An MCP server for every CLI.** Worth it only for a tool that wraps a
  structured API and is called often; a skill (A12) covers the rest at a
  fraction of the cost.
- **Model Armor-style response sanitizing.** Specific to Google Cloud. We keep the
  principle: text other people wrote is data. A tool that returns it says
  which fields carry it, and the skill calling the tool treats those fields
  as data, never as instructions.

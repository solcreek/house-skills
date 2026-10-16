# Rust

Template: `templates/rust/` — `src/main.rs` and `tests/contract.rs`
(`assert_cmd`). Scaffold adds `clap` (derive), `serde_json`, `anyhow`.

## Why it is shaped this way

- **clap** already does most of the must rules. `--help`/`-h` go to stdout
  with exit 0, and `--version` comes from `#[command(version)]`. An unknown
  flag or a missing argument is exit 2, with a short message and a pointer
  to `--help`, on stderr. Do not catch clap's errors to restyle them; that is
  how you lose the exit code.
- **Exit codes**: clap's 2 for usage, and `main` maps any `anyhow` error to
  1. Add codes as `u8` constants returned from `run`, and list them in the
  top-level doc comment (it is the help text).
- **`describe`** walks `Cli::command()` after `build()`, which adds
  `--help`/`--version` and copies global flags down, so it lists what help
  shows (A10). It skips clap's own `help` subcommand. Add `mutates` to a
  command's entry when you add one that mutates.
- **Colour**: clap's output honours `NO_COLOR` and the terminal. For your own
  output use `anstream` and `anstyle` (clap's dependencies) rather than
  writing escape codes.
- **`--json` is global**, so every subcommand accepts it. A command that
  reports nothing should say so under `--json` rather than print prose.
- **`main` parses through `command()`**, the derived command plus what derive
  cannot hold: each command's examples (from `EXAMPLES`) and the exit codes
  on every help page (E4). A usage error under `--json` is JSON too (A5),
  its message joined up to clap's first blank line so it names the flag.
- **No command prints help to stdout and exits 0** (`command: Option<…>`),
  where clap's default is the whole help on stderr with exit 2 (D3).
- **`--version zz`** prints the version and ignores the rest: clap's
  behaviour, harmless, and the one place the Rust template differs.
- **Lists flatten `Limit`** (`--limit N`, default 20; the value name `N`
  makes `describe` say `integer`) and go through `capped`. A partial result
  returns `ExitCode::from(PARTIAL)` after printing `problems`.
- **Mutating commands flatten `Change`**: an `ArgGroup` that is required and
  not multiple, so clap refuses neither and both with exit 2. `describe`
  marks a command `mutates` when it has `--yes`.
- **Tests** use `tempfile` for a fresh directory per case, with `TOOL_HOME`
  pointing inside it, and run in parallel (cargo's default).

## Distribution

`cargo install --path .` for the team. For release binaries, use `dist`
(cargo-dist): `dist init` writes a CI workflow that builds per-target
archives and an installer script. For Homebrew, `dist` can publish a tap.

## Tests

`assert_cmd` runs the built binary. Add a `trycmd` (or `snapbox`) snapshot
of `--help` once the help is stable, so a help change shows up in review.

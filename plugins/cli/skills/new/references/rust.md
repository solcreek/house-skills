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
- **`main` uses `try_parse`**, so a usage error under `--json` is JSON too
  (A5). It lets clap print help and version itself.
- **Mutating commands flatten `Change`**: an `ArgGroup` that is required and
  not multiple, so clap refuses neither and both with exit 2. `describe`
  marks a command `mutates` when it has `--yes`.
- **Tests** use `tempfile` for a fresh directory per case, with `TOOL_HOME`
  pointing inside it.

## Distribution

`cargo install --path .` for the team. For release binaries, use `dist`
(cargo-dist): `dist init` writes a CI workflow that builds per-target
archives and an installer script. For Homebrew, `dist` can publish a tap.

## Tests

`assert_cmd` runs the built binary. Add a `trycmd` (or `snapbox`) snapshot
of `--help` once the help is stable, so a help change shows up in review.

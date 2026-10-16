# Go

Template: `templates/go/` — `main.go` and `main_test.go` (builds the binary
once, runs it). Scaffold adds `github.com/spf13/cobra`.

## cobra's defaults break the rubric; the template fixes them

| default | breaks | template |
|---|---|---|
| A root without `RunE` answers `tool nope` with help and exit 0 | S2 | root `RunE` prints help; `Args: usageArgs(cobra.NoArgs)` |
| Every error prints the whole usage | D3 | `SilenceUsage`, `SilenceErrors`; `main` prints one line and points to `--help` |
| A bad flag or argument count exits 1, like any failure | E3 | `SetFlagErrorFunc` and `usageArgs` wrap them as `usageError`, which exits 2 |

Keep all three when adding commands. Wrap every `Args:` in `usageArgs(...)`.
`TestUsageErrorsExit2` fails if you forget.

## Other notes

- **Mutating commands**: `mutating(cmd)` adds `--dry-run` and `--yes` and
  marks it for `describe`. The command's `RunE` calls `change(cmd)` first,
  which returns a usage error unless exactly one is set.
- **Help on stdout**: `main` calls `root.SetOut(os.Stdout)`. Write command
  output with `cmd.OutOrStdout()` so tests can capture it.
- **`--json` is a persistent flag** on the root, so every command has it.
- **`describe`** walks the cobra tree (A10). It leaves out `completion` and
  hidden commands. It calls `InitDefaultHelpFlag`/`InitDefaultVersionFlag`
  first and reads `LocalFlags` plus `InheritedFlags`: `Flags()` alone misses
  `--help`, `--version` and the persistent `--json` on every command that is
  not running.
- **`--json` on a failure**: pflag stops at a bad flag, so `greet --zz --json`
  never sets `asJSON`. `main` also looks for `--json` in `os.Args` on the
  error path; keep that.
- **Colour**: `fatih/color` honours `NO_COLOR` and the terminal. cobra's
  `completion` command comes free; keep it (D5).
- **Version**: `var version` is set at release with
  `-ldflags "-X main.version=$(git describe --tags)"`.
- **Small tools**: the standard `flag` package exits 2 on a bad flag, but it
  writes `-h` output to stderr and dumps the full usage on errors. Set
  `flag.CommandLine.SetOutput(os.Stdout)` for help and a short
  `flag.Usage`, or use cobra from the start.

## Distribution

`goreleaser`: `goreleaser init`, then a tag-triggered workflow builds
archives per OS and architecture, and can publish a Homebrew tap.

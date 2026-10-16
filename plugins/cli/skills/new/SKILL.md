---
name: new
description: "Build a new command-line tool from scratch in TypeScript, Rust or Go that is safe and predictable for people and AI agents from its first commit: help before any action, usage errors with their own exit code, --json on every reporting command, a describe command for runtime introspection, contract tests, and an agent skill for it. Designs the command surface first, scaffolds from a template that passes cli:audit, then verifies with its probe. Use when asked to create, start, scaffold or design a CLI, a bin/ script with subcommands, or a tool agents will call."
argument-hint: "[what the tool does]"
allowed-tools:
  - Bash(${CLAUDE_SKILL_DIR}/scripts/*)
  - Bash(${CLAUDE_SKILL_DIR}/../audit/scripts/probe *)
---

# New CLI

Build a tool that passes [the rubric](../audit/references/rubric.md) on its
first commit. The templates already pass: every **must** rule, plus `--json`,
`describe` and a short usage error. Your job is to keep them passing while
you replace the example command with the real ones.

`$S` is `${CLAUDE_SKILL_DIR}/scripts`. `$A` is
`${CLAUDE_SKILL_DIR}/../audit/scripts`.

## 1. Pick the stack

Use the language of the repository the tool lives in. When that does not
decide it:

- **TypeScript** (default): a repo tool or a team tool on Node. Zero
  dependencies (`node:util` `parseArgs`); runs `.ts` directly on Node 22.18
  or later.
- **Rust**: a single static binary to distribute, or work that is heavy on
  CPU or startup time. Uses clap.
- **Go**: a single binary that is easy to cross-compile, or a team that
  already writes Go. Uses cobra.

Ask only if the repository and the request leave it open. Read
`references/<stack>.md` before writing code in that stack.

## 2. Design the surface before the code

Write the help first, as plain text in your reply or in a design note: every
command with its usage line, flags, one example, whether it **mutates**, and
the exit codes beyond 0/1/2. Check the design against the rubric now, when a
change is cheap:

- One verb per idea, kebab-case long flags, and the same flag means the same
  thing everywhere (D4).
- Every command that reports something has `--json`. Decide its fields now,
  and treat them as an API from then on (A1).
- Every mutating command takes `--yes` and `--dry-run`, and never prompts
  without a TTY (S3).
- Lists have a cap and a total (A6). "Could not look" has its own exit code
  or state, distinct from "found nothing" (A7).
- An argument that becomes a path, URL or query is validated (A9).
- Secrets come from env or stdin, never from a flag (S4).
- A tool wrapping an API also takes the whole request as JSON (A11).

## 3. Scaffold

    $S/scaffold STACK DIR NAME

This copies the template, renames the placeholder to NAME, and adds the
latest dependencies. It prints JSON with `dir`, `files`, `build` and `test`
(run both in `dir`), and `binary`, the absolute path to probe. Exit 2 means
it could not start (`error` says why: a missing toolchain, DIR not empty, a
name cargo reserves); exit 1 means a step failed (`log` has the tail, and a
DIR it created is gone, so you can rerun it).

For a tool inside an existing repository, scaffold into the directory where
it will live, then fold the files into the repository's own layout and test
runner. Keep the contract tests.

## 4. Replace the example

The template has one command, `greet`. Replace it with the commands from
step 2, keeping:

- the **help check before `run`** (TypeScript) and the usage-error mapping
  (Go). Neither is something the library does for you;
- `describe`, generated from the same definitions as help, in the shape of
  [references/describe.md](references/describe.md) shared by all three
  templates;
- the exit-code table, documented in the top-level help. Add new codes to it,
  and never reuse one;
- the contract tests, extended. For each command add a test that it rejects
  an unknown flag, and that `--json` output is a single document; for each
  mutating command, that it neither acts nor prompts without `--yes`.

## 5. Verify

Run `build` and `test` from step 3 in `dir`, then probe the top level and
every command:

    $A/probe -- BINARY
    $A/probe -- BINARY COMMAND                 # add --skip unknown-command when
                                               # COMMAND takes a free positional
    $A/run-json -- BINARY describe
    $A/run-json -- BINARY COMMAND … --json     # read-only commands; --lines for NDJSON

`run-json` runs the command, so it asks for approval: run it only on
commands that are read-only and local.

Every check must pass. Fix the code, not the check. The expected skips are:
`version` on a subcommand, `unknown-command` where a command takes a free
positional, and `no-color-env` when the tool never colours. Any other skip
needs a reason in your report.

## 6. Ship the agent context

Rubric A12: agents learn a tool from what is in their context, not from
`--help`. Add a short section to the repository's `AGENTS.md`, or a skill if
the tool is used across repositories. The section names the commands that
mutate, says to run `--dry-run` before them and to pass `--json`, and lists
what each exit code means. Point it at `describe` for everything else rather
than copying the flags, which would drift.

Report: the files created, the test and probe results (each `summary`), and
the design decisions from step 2 that the user should confirm.

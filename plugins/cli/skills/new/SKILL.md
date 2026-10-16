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
first commit and follows [the conventions](../audit/references/conventions.md)
our tools share: exit codes, flag names, JSON shapes, paths, state. The
templates already do both. Your job is to keep it so while you replace the
example commands with the real ones.

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

First list what the request leaves open about **behaviour**: what a command
reads and writes, what counts as done, where state lives, what happens on a
rerun. Ask the user those questions together, in one message. If you cannot
ask, pick the default the conventions suggest and list each choice under
"decisions to confirm" in your report.

Then write the help, as plain text in your reply or in a design note: every
command with its usage line, flags, one example, whether it **mutates**, and
the exit codes beyond 0/1/2. Check the design against the rubric now, when a
change is cheap:

- One verb per idea, kebab-case long flags, and the same flag means the same
  thing everywhere (D4).
- Every command that reports something has `--json`. Decide its fields now,
  and treat them as an API from then on (A1).
- Every command that changes anything besides its output is **mutating**:
  it requires `--dry-run` or `--yes`, and never prompts (S3, conventions).
- Lists use `--limit` and the `items`/`total`/`shown`/`truncated` shape
  (A6). A partial result exits 3 with `problems` (A7).
- State lives under XDG paths and moves wholesale with `NAME_HOME`, which the
  tests set (C2, conventions).
- An argument that becomes a path, URL or query is validated (A9). Output
  paths stay inside the root the command writes to.
- Secrets come from env or stdin, never from a flag (S4).
- A tool wrapping an API also takes the whole request as JSON (A11).

## 3. Scaffold

    $S/scaffold STACK DIR NAME

This copies the template, renames the placeholder to NAME, and adds the
latest dependencies. It prints JSON with `dir`, `files`, `build` and `test`
(run both in `dir`; `build` is null for TypeScript, which runs from source),
and `binary`, the absolute path to probe. Exit 2 means
it could not start (`error` says why: a missing toolchain, DIR not empty, a
name cargo reserves); exit 1 means a step failed (`log` has the tail, and a
DIR it created is gone, so you can rerun it).

For a tool inside an existing repository, scaffold into the directory where
it will live, then fold the files into the repository's own layout and test
runner. Keep the contract tests.

## 4. Replace the example

The template has two example commands, each a pattern to copy:

- `files [DIR] [--limit N]` reports: an optional positional with a default,
  an integer flag, the capped list shape with its stderr note, a missing
  input as failed, and unreadable parts as a partial result (exit 3).
- `touch PATH` mutates: `--dry-run`/`--yes`, an output path checked by
  `inside`, a rerun that changes nothing, and state written atomically under
  `home("state")`.

Model your commands on them, then delete them. Keep:

- the **help check before `run`**, the positional-count check and the
  `--dry-run`/`--yes` check in TypeScript's `main`, and the usage-error
  mapping in Go. The libraries do none of these for you;
- the mutating pattern: `mutates: true` (TypeScript), flattening `Change`
  (Rust), or `mutating(cmd)` plus `change(cmd)` (Go);
- the helpers: `inside` for any path the tool writes (create with
  create-only-if-absent), `write_atomic`/`writeAtomic` to replace a file,
  `home` for config, cache and state, and `capped` for every list;
- `describe`, generated from the same definitions as help, in the shape of
  [references/describe.md](references/describe.md) shared by all three
  templates;
- the exit-code table, documented in the top-level help. Add new codes to it,
  and never reuse one;
- the contract tests, extended: they run each case in a fresh directory
  with `TOOL_HOME` (renamed for your tool) pointing at a temporary one. For
  each command add a test that it rejects an unknown flag and extra
  arguments, and that `--json` output is a single document. For each
  mutating command, test that it neither acts nor prompts without `--yes`,
  that `--dry-run` changes nothing, and that a rerun succeeds. Give a
  mutating command `--dry-run` in its other tests (extra arguments, unknown
  flags): without it, the missing-flag check answers first and the test
  passes for the wrong reason.

## 5. Verify

Run `build` and `test` from step 3 in `dir`, then probe the top level and
every command:

    $A/probe -- BINARY
    $A/probe -- BINARY COMMAND                 # add --skip unknown-command when
                                               # COMMAND takes a free positional
    $A/run-json -- BINARY describe
    $A/run-json --expect-exit 0 --cwd FIXTURE --env NAME_HOME=$(mktemp -d) -- BINARY COMMAND … --json
    $A/run-json --expect-exit 2 -- BINARY MUTATING … --json   # no --yes: refused, as JSON

Give every `run-json` call an `--expect-exit`: without it, a failure that
prints a well-formed error passes. Point commands that read files at a
fixture with some content (`--cwd`, or an absolute path): in the default
empty directory they find nothing and pass trivially.

`run-json` runs the command, so it asks for approval: run it only on
read-only commands, and on mutating ones only with `--dry-run`. Add
`--lines` for NDJSON.

Every check must pass. Fix the code, not the check. The expected skips are:
`version` on a subcommand, `unknown-command` where a command takes a free
positional, and `no-color-env` when the tool never colours. Any other skip
needs a reason in your report.

## 6. Ship the agent context

Rubric A12: agents learn a tool from what is in their context, not from
`--help`. Fill in the "Agent context" section from the conventions and put
it in the repository's `AGENTS.md`, or in the tool's own `AGENTS.md` when it
is not part of a repository, or in a skill when it is used across
repositories. It names the commands that mutate, the exit codes, and the
fields that carry other people's text, and it points at `describe` for
everything else rather than copying the flags, which would drift.

Report: the files created, the test and probe results (each `summary`), and
the design decisions from step 2 that the user should confirm.

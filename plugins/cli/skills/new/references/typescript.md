# TypeScript

Template: `templates/typescript/` — `bin/NAME` (a bash shim), `src/main.ts`,
`src/main.test.ts` (`node:test`), `package.json`. No dependencies.

## Why it is shaped this way

- **`node:util` `parseArgs` with `strict: true`** rejects unknown flags
  (`ERR_PARSE_ARGS_UNKNOWN_OPTION`); `main` turns that into a usage error,
  exit 2. Do not filter `argv` by hand (`argv.includes("--json")`): that is
  how an unknown flag, or `--help`, reaches an action (S1, S2).
- **One command table** drives `help`, each command's `--help` and
  `describe`, so they cannot drift (A10). A command is data plus `run`.
- **Help is checked before `run`**, in `main`. `parseArgs` does not know
  `--help` is special.
- **`styleText(…, { stream })`** colours only when that stream is a terminal
  and `NO_COLOR` is unset, on every Node the template supports. Never write
  raw escape codes.
- **`.ts` runs directly** (type stripping, Node 22.18 or later): no build
  step for a repository tool. Only erasable syntax works: no `enum`, no
  `namespace`, no parameter properties. Use `as const` objects, as `EXIT`
  does.

## Startup (P1)

Node alone takes about 110–140 ms to start on s-imac (an Intel i9), and the
template's `--help` takes 164 ms (both measured 2026-10-15). Keep `--help`
under 300 ms by importing heavy
modules inside `run` (`const { x } = await import("./x.ts")`), never at the
top of `main.ts`.

## Publishing to npm

Type stripping is disabled for files under `node_modules`, so a published
package must ship JavaScript. Add `tsc` (or `tsdown`) to emit `dist/`, point
`bin` at a `dist/main.js` that starts with `#!/usr/bin/env node`, and add
`"files": ["dist"]`. Keep the shim for development only.

## Growing past one file

Move each command to `src/commands/<name>.ts`, exporting its `Command`, and
import them into the table. Once it has nested subcommand groups (three
levels or more), consider `citty`. Whatever you use, keep the exit codes
(commander, for one, exits 1 on a usage error unless you call
`exitOverride()`), and rerun the probe.

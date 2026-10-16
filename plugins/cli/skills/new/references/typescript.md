# TypeScript

Template: `templates/typescript/` — `bin/NAME` (the entry point), `src/main.ts`,
`src/main.test.ts` (`node:test`), `package.json`. No dependencies.

## Why it is shaped this way

- **`node:util` `parseArgs` with `strict: true`** rejects unknown flags
  (`ERR_PARSE_ARGS_UNKNOWN_OPTION`); `main` turns that into a usage error,
  exit 2. Do not filter `argv` by hand (`argv.includes("--json")`): that is
  how an unknown flag, or `--help`, reaches an action (S1, S2).
- **One command table** drives `help`, each command's `--help` and
  `describe`, so they cannot drift (A10). A command is data plus `run`:
  `args` declares its positionals, `mutates` its kind, `options` its flags
  (with `value` for the placeholder in help and `default`).
- **`main` checks, in order, before `run`**: help, the positional count
  (too few or too many is usage), and for a mutating command exactly one of
  `--dry-run`/`--yes`. `parseArgs` does none of this; its own errors are
  folded to one line.
- **Errors**: throw `UsageError` for a bad invocation (exit 2) and
  `Failure` for an expected failure (exit 1, message only). Anything else is
  a bug: exit 1 with the stack.
- **`styleText(…, { stream })`** colours only when that stream is a terminal
  and `NO_COLOR` is unset, on every Node the template supports. Never write
  raw escape codes.
- **`.ts` runs directly** (type stripping, Node 22.18 or later): no build
  step for a repository tool. Only erasable syntax works: no `enum`, no
  `namespace`, no parameter properties. Use `as const` objects, as `EXIT`
  does. `import type { … } from "./x.ts"` is erasable, which is how a
  lazily imported module shares its types.
- **Tests** spawn the CLI once per case with an asynchronous `run()` and
  `runAll()`, so they really run concurrently (spawnSync would serialize
  them). Each case gets a fresh directory, stdin at EOF, a timeout and
  `TOOL_HOME`; pass `cwd` or `env` to share them across steps.
- **Integer flags**: give an option `value: "N"`; `main` rejects anything
  but a whole number, and `describe` says `integer`. `default` fills the
  value when the flag is absent.

## Startup (P1)

`bin/tool` is a small ES module, not a shell script: it turns on Node's
compile cache (in the system temp directory) and imports `src/main.ts`.
Skipping bash and reusing compiled code took `--help` on s-imac from about
300 ms to 80–110 ms on Node 24, and 170 ms on Node 22.18, measured
2026-10-15 under a load average of 40–50. Keep `--help` under 300 ms by
importing heavy modules inside `run` (`const { x } = await import("./x.ts")`),
never at the top of `main.ts`.

## Publishing to npm

Type stripping is disabled for files under `node_modules`, so a published
package must ship JavaScript. Add `tsc` (or `tsdown`) to emit `dist/`, point
`bin` at a `dist/main.js` that starts with `#!/usr/bin/env node`, and add
`"files": ["dist"]`. Keep `bin/NAME` importing `src/main.ts` for development only.

## Growing past one file

Move each command to `src/commands/<name>.ts`, exporting its `Command`, and
import them into the table. Once it has nested subcommand groups (three
levels or more), consider `citty`. Whatever you use, keep the exit codes
(commander, for one, exits 1 on a usage error unless you call
`exitOverride()`), and rerun the probe.

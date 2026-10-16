import { existsSync, lstatSync, realpathSync, writeFileSync } from "node:fs";
import { basename, dirname, isAbsolute, join, relative, resolve, sep } from "node:path";
import { parseArgs, styleText } from "node:util";

// Conventions: ../audit/references/conventions.md in the cli plugin.
const NAME = "tool";
const VERSION = "0.1.0";
const SUMMARY = "one line on what it is for";

// Every exit code, in help and in `describe`. Add to it; never reuse one.
const EXIT = { ok: 0, failed: 1, usage: 2 } as const;

class UsageError extends Error {}
class Failure extends Error {}

type Option = {
  type: "boolean" | "string";
  short?: string;
  value?: string; // the placeholder in help: --limit N
  default?: string;
  description: string;
};
type Values = Record<string, string | boolean | undefined>;
type Arg = { name: string; required: boolean };
type Command = {
  name: string;
  summary: string;
  args: Arg[]; // positionals, checked before run: too few or too many is usage
  examples: string[];
  // Changes anything besides its own output. Gets --dry-run and --yes, and
  // main refuses to run it without exactly one of them.
  mutates: boolean;
  options: Record<string, Option>;
  run(values: Values, args: string[]): number | Promise<number>;
};

const COMMON: Record<string, Option> = {
  json: { type: "boolean", description: "print one JSON document on stdout" },
  help: { type: "boolean", short: "h", description: "show this help" },
};
const MUTATING: Record<string, Option> = {
  "dry-run": { type: "boolean", description: "show what would change; change nothing" },
  yes: { type: "boolean", description: "apply the change" },
};

const optionsOf = (c: Command) => ({ ...c.options, ...(c.mutates ? MUTATING : {}), ...COMMON });
const usageOf = (c: Command) =>
  [NAME, c.name, ...c.args.map((a) => (a.required ? a.name : `[${a.name}]`)), "[options]"].join(" ");

// Colour only where it helps a person: styleText drops it for a non-terminal
// stream and under NO_COLOR.
const bold = (s: string) => styleText("bold", s, { stream: process.stdout });

function print(values: Values, json: unknown, text: string) {
  console.log(values.json ? JSON.stringify(json) : text);
}

// An empty argument or a control character in one is a usage error (rubric A9).
function plain(what: string, s: string): string {
  if (s === "") throw new UsageError(`${what} is empty`);
  if (/[\u0000-\u001f\u007f]/.test(s)) throw new UsageError(`${what} contains a control character`);
  return s;
}

// The file an output path names, refused unless it is plainly inside root
// (rubric A9). Every template's inside() refuses the same things: a `..`
// part, a trailing slash, the root itself, a parent that resolves outside
// root, and a last part that is a symlink (dangling or not), so the write
// can never follow one.
function inside(root: string, path: string): string {
  const refuse = (why: string) => new Failure(`${path}: ${why}`);
  if (path.split(/[\\/]/).includes("..")) throw refuse("'..' is not allowed in an output path");
  if (/[\\/]$/.test(path)) throw refuse("names a directory, not a file");
  const realRoot = realpathSync(root);
  const parent = dirname(isAbsolute(path) ? path : join(root, path));
  if (!existsSync(parent)) throw refuse(`no such directory: ${parent}`);
  const target = join(realpathSync(parent), basename(path));
  const rel = relative(realRoot, target);
  if (rel === "" || rel === ".") throw refuse(`is ${realRoot} itself`);
  if (rel === ".." || rel.startsWith(`..${sep}`) || isAbsolute(rel))
    throw refuse(`outside ${realRoot}; ${NAME} writes only under the current directory`);
  let link = false;
  try {
    link = lstatSync(target).isSymbolicLink();
  } catch {} // absent: fine
  if (link) throw refuse("is a symlink; refusing to write through it");
  return target;
}

const commands: Command[] = [
  {
    name: "greet",
    summary: "print a greeting",
    args: [{ name: "NAME", required: true }],
    examples: [`${NAME} greet Ada`, `${NAME} greet Ada --json`],
    mutates: false,
    options: {},
    run(values, [who]) {
      const greeting = `hello, ${plain("NAME", who)}`;
      print(values, { greeting }, greeting);
      return EXIT.ok;
    },
  },
  {
    name: "touch",
    summary: "create an empty file under the current directory",
    args: [{ name: "PATH", required: true }],
    examples: [`${NAME} touch notes.txt --dry-run`, `${NAME} touch notes.txt --yes --json`],
    mutates: true,
    options: {},
    run(values, [path]) {
      const target = inside(process.cwd(), plain("PATH", path));
      const dryRun = Boolean(values["dry-run"]);
      // Already there is success with nothing changed (rubric S5).
      const changed = !existsSync(target);
      if (changed && !dryRun) {
        try {
          writeFileSync(target, "", { flag: "wx" });
        } catch (e) {
          // Expected (permissions, a race): a failure naming the file, not a bug.
          throw new Failure(`cannot create ${target}: ${(e as { code?: string }).code ?? e}`);
        }
      }
      const verb = !changed ? "exists" : dryRun ? "would create" : "created";
      print(values, { path: target, changed, dryRun }, `${verb} ${target}`);
      return EXIT.ok;
    },
  },
  {
    name: "describe",
    summary: "print every command, flag and exit code as JSON",
    args: [],
    examples: [`${NAME} describe`],
    mutates: false,
    options: {},
    run() {
      console.log(JSON.stringify(describe(), null, 2));
      return EXIT.ok;
    },
  },
];

function exitCodes(): string {
  return Object.entries(EXIT).map(([k, v]) => `${v} ${k}`).join(", ");
}

// Usage and summary on separate lines: long usages never push help past 100
// columns (rubric D2).
function help(): string {
  return [
    `${NAME}: ${SUMMARY}`,
    "",
    `${bold("Usage:")} ${NAME} COMMAND [options]`,
    "",
    bold("Commands:"),
    ...commands.flatMap((c) => [
      `  ${usageOf(c).slice(NAME.length + 1)}`,
      `      ${c.summary}${c.mutates ? " (changes things)" : ""}`,
    ]),
    "",
    `Run '${NAME} COMMAND --help' for a command's options and examples,`,
    `or '${NAME} describe' for all of it as JSON.`,
    "",
    `Exit codes: ${exitCodes()}`,
  ].join("\n");
}

function commandHelp(c: Command): string {
  const rows = Object.entries(optionsOf(c)).map(([k, o]) => [
    `${o.short ? `-${o.short}, ` : "    "}--${k}${o.type === "string" ? ` ${o.value ?? "VALUE"}` : ""}`,
    o.description + (o.default === undefined ? "" : ` (default ${o.default})`),
  ]);
  const width = Math.max(...rows.map(([flag]) => flag.length));
  return [
    `${bold("Usage:")} ${usageOf(c)}`,
    "",
    `${c.summary[0].toUpperCase()}${c.summary.slice(1)}.`,
    ...(c.mutates ? ["Changes things: pass --dry-run to preview, --yes to apply."] : []),
    "",
    bold("Options:"),
    ...rows.map(([flag, text]) => `  ${flag.padEnd(width)}  ${text}`),
    "",
    bold("Examples:"),
    ...c.examples.map((e) => `  ${e}`),
    "",
    `Exit codes: ${exitCodes()}`,
  ].join("\n");
}

// The same definitions as --help, as data (rubric A10), in the shape every
// template's describe prints: see references/describe.md.
function flagList(options: Record<string, Option>) {
  return Object.entries(options).map(([name, o]) => ({
    name,
    short: o.short ?? null,
    takesValue: o.type === "string",
    default: o.default ?? null,
    description: o.description,
  }));
}

function describe() {
  return {
    name: NAME,
    version: VERSION,
    summary: SUMMARY,
    exitCodes: EXIT,
    flags: flagList({
      json: COMMON.json,
      help: COMMON.help,
      version: { type: "boolean", description: "print the version" },
    }),
    commands: commands.map((c) => ({
      name: c.name,
      summary: c.summary,
      usage: usageOf(c),
      mutates: c.mutates,
      examples: c.examples,
      flags: flagList(optionsOf(c)),
      commands: [],
    })),
  };
}

async function main(args: string[]): Promise<number> {
  let argv = args;
  // --json is global, as in the Rust and Go templates: `tool --json greet`.
  if (argv[0] === "--json" && argv.length > 1) argv = [argv[1], "--json", ...argv.slice(2)];
  const [first, ...rest] = argv;
  if (first === undefined || first === "-h" || first === "--help" || first === "help") {
    // `help COMMAND` is that command's help, as clap and cobra do.
    if (first === "help" && rest.length === 1) argv = [rest[0], "--help"];
    else {
      if (rest.length > 0) throw new UsageError(`${first} takes at most one command`);
      console.log(help());
      return EXIT.ok;
    }
  }
  return dispatch(argv);
}

async function dispatch([first, ...rest]: string[]): Promise<number> {
  if (first === "--version") {
    if (rest.length > 0) throw new UsageError("--version takes no arguments");
    console.log(`${NAME} ${VERSION}`);
    return EXIT.ok;
  }
  const command = commands.find((c) => c.name === first);
  if (!command)
    throw new UsageError(`unknown ${first.startsWith("-") ? "flag" : "command"} "${first}"`);
  let parsed;
  try {
    parsed = parseArgs({ args: rest, options: optionsOf(command), strict: true, allowPositionals: true });
  } catch (e) {
    // ERR_PARSE_ARGS_UNKNOWN_OPTION and friends: a usage error, never
    // ignored, folded to one line (rubric D3).
    throw new UsageError((e as Error).message.split("\n")[0]);
  }
  const { values, positionals } = parsed;
  // Help before anything else the command does (rubric S1).
  if (values.help) {
    console.log(commandHelp(command));
    return EXIT.ok;
  }
  const required = command.args.filter((a) => a.required).length;
  if (positionals.length < required || positionals.length > command.args.length) {
    const want = command.args.map((a) => a.name).join(" ") || "no arguments";
    throw new UsageError(`${command.name} takes ${want}; got ${positionals.length}`);
  }
  if (command.mutates && Boolean(values["dry-run"]) === Boolean(values.yes))
    throw new UsageError(`${command.name} changes things: pass --dry-run to preview, or --yes to apply (one of them)`);
  return command.run(values, positionals);
}

const argv = process.argv.slice(2);
try {
  process.exitCode = await main(argv);
} catch (e) {
  const usage = e instanceof UsageError;
  const message = e instanceof Error ? e.message : String(e);
  // A Failure is expected and its message is enough; anything else is a bug,
  // so keep the stack for whoever reports it.
  if (!(usage || e instanceof Failure) && e instanceof Error && e.stack) console.error(e.stack);
  console.error(`${NAME}: ${message}`);
  if (usage) console.error(`Run '${NAME} --help' for usage.`);
  // Under --json a failure is JSON too (rubric A5). The only place argv is
  // searched by hand: parsing may have failed before it saw --json.
  if (argv.includes("--json")) console.log(JSON.stringify({ error: message, usage }));
  process.exitCode = usage ? EXIT.usage : EXIT.failed;
}

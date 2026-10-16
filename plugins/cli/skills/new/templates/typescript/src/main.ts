import { parseArgs, styleText } from "node:util";

const NAME = "tool";
const VERSION = "0.1.0";
const SUMMARY = "one line on what it is for";

// Every exit code, in help and in `describe`. Add to it; never reuse one.
const EXIT = { ok: 0, failed: 1, usage: 2 } as const;

class UsageError extends Error {}

type Option = { type: "boolean" | "string"; short?: string; description: string };
type Values = Record<string, string | boolean | undefined>;
type Command = {
  name: string;
  summary: string;
  usage: string;
  examples: string[];
  mutates: boolean; // true: needs --yes, and has --dry-run
  options: Record<string, Option>;
  run(values: Values, positionals: string[]): number | Promise<number>;
};

const COMMON: Record<string, Option> = {
  help: { type: "boolean", short: "h", description: "show this help" },
  json: { type: "boolean", description: "print one JSON document on stdout" },
};

const commands: Command[] = [
  {
    name: "greet",
    summary: "print a greeting",
    usage: "greet NAME [--json]",
    examples: [`${NAME} greet Ada`, `${NAME} greet Ada --json`],
    mutates: false,
    options: {},
    run(values, [who]) {
      if (!who) throw new UsageError("greet needs a NAME");
      const greeting = `hello, ${who}`;
      if (values.json) console.log(JSON.stringify({ greeting }));
      else console.log(styleText("bold", greeting, { stream: process.stdout }));
      return EXIT.ok;
    },
  },
];

function help(): string {
  const width = Math.max(...commands.map((c) => c.usage.length));
  return [
    `${NAME}: ${SUMMARY}`,
    "",
    "Usage:",
    ...commands.map((c) => `  ${NAME} ${c.usage.padEnd(width)}  ${c.summary}`),
    "",
    `Run '${NAME} COMMAND --help' for a command's flags and examples,`,
    `or '${NAME} describe' for all of it as JSON.`,
    "",
    `Exit codes: ${Object.entries(EXIT).map(([k, v]) => `${v} ${k}`).join(", ")}`,
  ].join("\n");
}

function commandHelp(c: Command): string {
  const opts = { ...c.options, ...COMMON };
  return [
    `${NAME} ${c.usage}`,
    "",
    `${c.summary[0].toUpperCase()}${c.summary.slice(1)}.`,
    "",
    "Options:",
    ...Object.entries(opts).map(
      ([k, o]) => `  ${o.short ? `-${o.short}, ` : "    "}--${k}${o.type === "string" ? " VALUE" : ""}  ${o.description}`,
    ),
    "",
    "Examples:",
    ...c.examples.map((e) => `  ${e}`),
  ].join("\n");
}

// The same definitions as --help, as data (rubric A10), in the shape every
// template's describe prints: see references/describe.md.
function flagList(options: Record<string, Option>) {
  return Object.entries(options).map(([name, o]) => ({
    name,
    short: o.short ?? null,
    takesValue: o.type === "string",
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
      help: COMMON.help,
      version: { type: "boolean", description: "print the version" },
    }),
    commands: commands.map((c) => ({
      name: c.name,
      summary: c.summary,
      usage: `${NAME} ${c.usage}`,
      mutates: c.mutates,
      examples: c.examples,
      flags: flagList({ ...c.options, ...COMMON }),
      commands: [],
    })),
  };
}

async function main(argv: string[]): Promise<number> {
  const [first, ...rest] = argv;
  if (first === undefined || first === "-h" || first === "--help" || first === "help") {
    console.log(help());
    return EXIT.ok;
  }
  if (first === "describe") {
    console.log(JSON.stringify(describe(), null, 2));
    return EXIT.ok;
  }
  if (first === "--version") {
    console.log(`${NAME} ${VERSION}`);
    return EXIT.ok;
  }
  const command = commands.find((c) => c.name === first);
  if (!command)
    throw new UsageError(`unknown ${first.startsWith("-") ? "flag" : "command"} "${first}"`);
  let parsed;
  try {
    parsed = parseArgs({
      args: rest,
      options: { ...command.options, ...COMMON },
      strict: true,
      allowPositionals: true,
    });
  } catch (e) {
    // ERR_PARSE_ARGS_UNKNOWN_OPTION and friends: a usage error, never ignored.
    throw new UsageError((e as Error).message);
  }
  // Help before anything else the command does (rubric S1).
  if (parsed.values.help) {
    console.log(commandHelp(command));
    return EXIT.ok;
  }
  return command.run(parsed.values, parsed.positionals);
}

const argv = process.argv.slice(2);
try {
  process.exitCode = await main(argv);
} catch (e) {
  const usage = e instanceof UsageError;
  const message = e instanceof Error ? e.message : String(e);
  console.error(`${NAME}: ${message}`);
  if (usage) console.error(`Run '${NAME} --help' for usage.`);
  // Under --json a failure is JSON too (rubric A5). The only place argv is
  // searched by hand: parsing may have failed before it saw --json.
  if (argv.includes("--json")) console.log(JSON.stringify({ error: message, usage }));
  process.exitCode = usage ? EXIT.usage : EXIT.failed;
}

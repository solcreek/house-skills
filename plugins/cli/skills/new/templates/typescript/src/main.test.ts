// The rubric's must rules, as tests. Run with `npm test`. Every case spawns
// the CLI; run() is asynchronous, so the cases really do run concurrently.
import assert from "node:assert/strict";
import { spawn } from "node:child_process";
import { chmodSync, existsSync, mkdirSync, mkdtempSync, readFileSync, symlinkSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { describe, test } from "node:test";
import { fileURLToPath } from "node:url";

const bin = fileURLToPath(new URL("../bin/tool", import.meta.url));
type Result = { out: string; err: string; code: number | null };
type Opts = { cwd?: string; env?: Record<string, string> };

// stdin is a pipe at EOF and there is a timeout, so a command that prompts
// fails instead of hanging. TOOL_HOME isolates state (conventions).
// Asynchronous on purpose: spawnSync would block the event loop and make
// the suite's concurrency a fiction.
function run(args: string[], opts: Opts = {}): Promise<Result> {
  return new Promise((done, fail) => {
    const child = spawn(bin, args, {
      cwd: opts.cwd ?? sandbox(),
      env: { ...process.env, NO_COLOR: "1", TOOL_HOME: sandbox(), ...opts.env },
    });
    let out = "";
    let err = "";
    child.stdout.setEncoding("utf8").on("data", (d) => (out += d));
    child.stderr.setEncoding("utf8").on("data", (d) => (err += d));
    child.stdin.end();
    const timer = setTimeout(() => child.kill("SIGKILL"), 10_000);
    child.on("error", fail);
    child.on("close", (code) => {
      clearTimeout(timer);
      done({ out, err, code });
    });
  });
}
// Every argv at once; results in the same order.
const runAll = (cases: string[][], opts?: Opts) => Promise.all(cases.map((a) => run(a, opts)));
const sandbox = () => mkdtempSync(join(tmpdir(), "cli-test-"));
const json = (r: Result) => JSON.parse(r.out);

// A directory with three files of different sizes.
function tree(): string {
  const d = sandbox();
  mkdirSync(join(d, "sub"));
  writeFileSync(join(d, "a"), "x".repeat(30));
  writeFileSync(join(d, "b"), "x".repeat(20));
  writeFileSync(join(d, "sub", "c"), "x".repeat(10));
  return d;
}

describe("contract", { concurrency: true }, () => {
  test("help goes to stdout and exits 0, before any action", async () => {
    const cases = [["--help"], ["-h"], ["help"], ["help", "touch"], ["files", "--help"], ["touch", "-h"], ["describe", "--help"]];
    for (const [i, r] of (await runAll(cases)).entries()) {
      assert.equal(r.code, 0, cases[i].join(" "));
      assert.match(r.out, /Usage:/, cases[i].join(" "));
      assert.equal(r.err, "", cases[i].join(" "));
    }
  });

  test("help needs no --yes, even on a mutating command", async () => {
    const cwd = sandbox();
    assert.equal((await run(["touch", "x", "--help"], { cwd })).code, 0);
    assert.equal(existsSync(join(cwd, "x")), false);
  });

  test("usage errors exit 2 with a short message on stderr", async () => {
    const cases = [
      ["--zz"], ["nope"], ["help", "zz"], ["help", "a", "b"], ["--version", "zz"], ["describe", "zz"],
      ["files", "a", "b"], ["files", "--zz"], ["files", "--limit", "x"], ["files", "--limit=-1"], ["files", "A\u0007"],
      // A mutating command gets --dry-run, or the missing-flag check would answer first.
      ["touch", "--dry-run"], ["touch", "a", "b", "--dry-run"], ["touch", "", "--dry-run"], ["touch", "x", "--dry-run", "--zz"],
    ];
    for (const [i, r] of (await runAll(cases)).entries()) {
      const what = cases[i].join(" ");
      assert.equal(r.code, 2, what);
      assert.equal(r.out, "", what);
      assert.match(r.err, /--help/, what);
      assert.ok(r.err.trim().split("\n").length <= 2, `${what}: ${r.err}`);
    }
  });

  test("--version prints a version", async () => {
    assert.match((await run(["--version"])).out, /^tool \d+\.\d+\.\d+\n$/);
  });

  test("--json is one document, before or after the command, failures included", async () => {
    const cwd = tree();
    const [after, before] = await runAll([["files", "--json"], ["--json", "files"]], { cwd });
    assert.deepEqual(json(after), json(before));
    for (const args of [["files", "a", "b", "--json"], ["files", "--zz", "--json"]]) {
      const r = await run(args);
      assert.equal(r.code, 2);
      assert.deepEqual(Object.keys(json(r)).sort(), ["error", "usage"]);
    }
  });

  test("a list is capped, says so on stderr, and has the list shape", async () => {
    const cwd = tree();
    const [capped, all, text] = await runAll(
      [["files", "--limit", "2", "--json"], ["files", "--limit", "0", "--json"], ["files", "--limit", "1"]],
      { cwd },
    );
    const c = json(capped);
    assert.deepEqual([c.total, c.shown, c.truncated], [3, 2, true]);
    assert.deepEqual(c.items.map((f: { path: string }) => f.path), ["a", "b"]);
    assert.match(capped.err, /2 of 3 shown; --limit 0 for all/);
    assert.deepEqual([json(all).shown, json(all).truncated, all.err], [3, false, ""]);
    assert.equal(text.out, "30\ta\n"); // stdout carries results only
  });

  test("a missing input is a failure that names it; an unreadable part is partial", async () => {
    const missing = await run(["files", "nowhere", "--json"]);
    assert.equal(missing.code, 1);
    assert.match(json(missing).error, /no such directory: .*nowhere/);
    if (process.getuid?.() === 0) return; // root reads everything
    const cwd = tree();
    chmodSync(join(cwd, "sub"), 0o000);
    const r = await run(["files", "--json"], { cwd });
    chmodSync(join(cwd, "sub"), 0o700);
    assert.equal(r.code, 3);
    assert.deepEqual(json(r).problems.map((p: { input: string }) => p.input), ["sub"]);
    assert.equal(json(r).total, 2); // the rest is still there
  });

  test("a mutating command neither acts nor prompts without --dry-run or --yes", async () => {
    for (const flags of [[], ["--dry-run", "--yes"]]) {
      const cwd = sandbox();
      const r = await run(["touch", "x", ...flags], { cwd });
      assert.equal(r.code, 2, flags.join(" "));
      assert.match(r.err, /--dry-run/);
      assert.equal(existsSync(join(cwd, "x")), false);
    }
  });

  test("--dry-run changes nothing; --yes applies and records state; a rerun is a success", async () => {
    const cwd = sandbox();
    const home = sandbox();
    const dry = json(await run(["touch", "x", "--dry-run", "--json"], { cwd, env: { TOOL_HOME: home } }));
    assert.deepEqual([dry.changed, dry.dryRun, existsSync(join(cwd, "x"))], [true, true, false]);
    assert.equal(existsSync(join(home, "state")), false);
    const yes = json(await run(["touch", "x", "--yes", "--json"], { cwd, env: { TOOL_HOME: home } }));
    assert.deepEqual([yes.changed, existsSync(join(cwd, "x"))], [true, true]);
    assert.deepEqual(JSON.parse(readFileSync(join(home, "state", "history.json"), "utf8")), [yes.path]);
    const again = await run(["touch", "x", "--yes", "--json"], { cwd, env: { TOOL_HOME: home } });
    assert.equal(again.code, 0);
    assert.equal(json(again).changed, false);
  });

  test("an output path cannot escape the current directory or follow a symlink", async () => {
    const outside = sandbox();
    writeFileSync(join(outside, "keep"), "precious");
    const cwd = sandbox();
    mkdirSync(join(cwd, "sub"));
    symlinkSync(outside, join(cwd, "out")); // a directory outside
    symlinkSync(join(outside, "keep"), join(cwd, "link")); // a file outside
    symlinkSync(join(outside, "gone"), join(cwd, "dangling"));
    symlinkSync(outside, join(cwd, "sub", "up"));
    const paths = ["../x", "/tmp/x", "out/x", "link", "dangling", "sub/up/../x", "sub/", `${cwd}/sub/`, ".", cwd, "newdir/"];
    const cases = paths.flatMap((p) => [["touch", p, "--dry-run", "--json"], ["touch", p, "--yes", "--json"]]);
    for (const [i, r] of (await runAll(cases, { cwd })).entries()) {
      assert.equal(r.code, 1, `${cases[i].join(" ")}: ${r.out}${r.err}`);
      assert.equal(json(r).usage, false, cases[i].join(" "));
    }
    assert.equal(readFileSync(join(outside, "keep"), "utf8"), "precious");
    assert.equal(existsSync(join(outside, "gone")) || existsSync(join(outside, "x")), false);
    assert.equal(existsSync(join(cwd, "sub", "sub")) || existsSync(join(cwd, "newdir")), false);
  });

  test("an absolute path inside the current directory is fine", async () => {
    const cwd = sandbox();
    const r = await run(["touch", join(cwd, "x"), "--yes", "--json"], { cwd });
    assert.equal(r.code, 0, r.err);
  });

  test("a write that fails names the file and is not a crash", async () => {
    if (process.getuid?.() === 0) return; // root writes anywhere
    const cwd = sandbox();
    mkdirSync(join(cwd, "ro"));
    chmodSync(join(cwd, "ro"), 0o500);
    const r = await run(["touch", "ro/x", "--yes", "--json"], { cwd });
    chmodSync(join(cwd, "ro"), 0o700);
    assert.equal(r.code, 1);
    assert.match(json(r).error, /cannot create .*ro\/x/);
    assert.doesNotMatch(r.err, /\n\s+at /); // no stack trace
  });

  test("describe lists every command, its args and flags as --help shows them", async () => {
    type Flag = { name: string };
    const d = json(await run(["describe"]));
    const names = (flags: Flag[]) => flags.map((f) => f.name).sort();
    assert.deepEqual(d.exitCodes, { ok: 0, failed: 1, usage: 2, partial: 3 });
    assert.deepEqual(names(d.flags), ["help", "json", "version"]);
    assert.deepEqual(Object.keys(d).sort(), ["commands", "exitCodes", "flags", "name", "summary", "version"]);
    const by = Object.fromEntries(d.commands.map((c: { name: string }) => [c.name, c]));
    assert.deepEqual(names(by.files.flags), ["help", "json", "limit"]);
    assert.deepEqual(by.files.args, [{ name: "DIR", required: false, default: "." }]);
    assert.deepEqual(
      by.files.flags.find((f: Flag) => f.name === "limit"),
      { name: "limit", short: null, type: "integer", default: "20", description: "show at most N items; 0 for all" },
    );
    assert.deepEqual(names(by.touch.flags), ["dry-run", "help", "json", "yes"]);
    assert.equal(by.touch.mutates, true);
    assert.ok(by.files.examples.length > 0);
  });
});

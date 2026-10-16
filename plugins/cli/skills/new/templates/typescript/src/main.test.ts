// The rubric's must rules, as tests. Run with `npm test`. Each case spawns
// the tool, so the suite runs them concurrently.
import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { existsSync, mkdtempSync, symlinkSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { describe, test } from "node:test";
import { fileURLToPath } from "node:url";

const bin = fileURLToPath(new URL("../bin/tool", import.meta.url));

// stdin is a pipe at EOF and there is a timeout, so a command that prompts
// fails instead of hanging. NAME_HOME isolates any state (conventions).
function run(args: string[], opts: { cwd?: string; env?: Record<string, string> } = {}) {
  const r = spawnSync(bin, args, {
    encoding: "utf8",
    input: "",
    timeout: 10_000,
    cwd: opts.cwd ?? mkdtempSync(join(tmpdir(), "tool-")),
    env: { ...process.env, NO_COLOR: "1", TOOL_HOME: mkdtempSync(join(tmpdir(), "tool-home-")), ...opts.env },
  });
  return { out: r.stdout, err: r.stderr, code: r.status };
}
const sandbox = () => mkdtempSync(join(tmpdir(), "tool-"));

describe("contract", { concurrency: true }, () => {
  test("help goes to stdout and exits 0, before any action", () => {
    for (const args of [["--help"], ["-h"], ["help"], ["greet", "--help"], ["touch", "-h"], ["describe", "--help"]]) {
      const r = run(args);
      assert.equal(r.code, 0, args.join(" "));
      assert.match(r.out, /Usage:/, args.join(" "));
      assert.equal(r.err, "", args.join(" "));
    }
  });

  test("help needs no --yes, even on a mutating command", () => {
    const cwd = sandbox();
    assert.equal(run(["touch", "x", "--help"], { cwd }).code, 0);
    assert.equal(existsSync(join(cwd, "x")), false);
  });

  test("usage errors exit 2 with a short message on stderr", () => {
    const cases = [
      ["--zz"], ["nope"], ["help", "zz"], ["--version", "zz"],
      ["greet"], ["greet", "Ada", "Bob"], ["greet", "Ada", "--zz"],
      ["describe", "zz"], ["greet", "A\u0007da"],
    ];
    for (const args of cases) {
      const r = run(args);
      assert.equal(r.code, 2, args.join(" "));
      assert.equal(r.out, "", args.join(" "));
      assert.match(r.err, /--help/, args.join(" "));
      assert.ok(r.err.trim().split("\n").length <= 2, `${args.join(" ")}: ${r.err}`);
    }
  });

  test("--version prints a version", () => {
    assert.match(run(["--version"]).out, /^tool \d+\.\d+\.\d+\n$/);
  });

  test("--json is one document, failures included", () => {
    assert.deepEqual(JSON.parse(run(["greet", "Ada", "--json"]).out), { greeting: "hello, Ada" });
    for (const args of [["greet", "--json"], ["greet", "--zz", "--json"]]) {
      const r = run(args);
      assert.equal(r.code, 2);
      assert.deepEqual(Object.keys(JSON.parse(r.out)).sort(), ["error", "usage"]);
    }
  });

  test("a mutating command neither acts nor prompts without --dry-run or --yes", () => {
    for (const flags of [[], ["--dry-run", "--yes"]]) {
      const cwd = sandbox();
      const r = run(["touch", "x", ...flags], { cwd });
      assert.equal(r.code, 2, flags.join(" "));
      assert.match(r.err, /--dry-run/);
      assert.equal(existsSync(join(cwd, "x")), false);
    }
  });

  test("--dry-run changes nothing; --yes applies; a rerun is a success", () => {
    const cwd = sandbox();
    const dry = JSON.parse(run(["touch", "x", "--dry-run", "--json"], { cwd }).out);
    assert.deepEqual([dry.changed, dry.dryRun, existsSync(join(cwd, "x"))], [true, true, false]);
    const yes = JSON.parse(run(["touch", "x", "--yes", "--json"], { cwd }).out);
    assert.deepEqual([yes.changed, existsSync(join(cwd, "x"))], [true, true]);
    const again = run(["touch", "x", "--yes", "--json"], { cwd });
    assert.equal(again.code, 0);
    assert.equal(JSON.parse(again.out).changed, false);
  });

  test("an output path cannot escape the current directory", () => {
    const cwd = sandbox();
    symlinkSync(tmpdir(), join(cwd, "out"));
    for (const path of ["../x", "/tmp/x", "out/x"]) {
      const r = run(["touch", path, "--yes", "--json"], { cwd });
      assert.equal(r.code, 1, path);
      assert.match(JSON.parse(r.out).error, /outside/, path);
    }
  });

  test("describe lists every command, with the flags --help shows", () => {
    type Flag = { name: string };
    const d = JSON.parse(run(["describe"]).out);
    const names = (flags: Flag[]) => flags.map((f) => f.name).sort();
    assert.equal(d.exitCodes.usage, 2);
    assert.ok(names(d.flags).includes("version"));
    const byName = Object.fromEntries(d.commands.map((c: { name: string }) => [c.name, c]));
    assert.deepEqual(names(byName.greet.flags), ["help", "json"]);
    assert.deepEqual(names(byName.touch.flags), ["dry-run", "help", "json", "yes"]);
    assert.equal(byName.touch.mutates, true);
  });
});

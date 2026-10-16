// The rubric's must rules, as tests. Run with `npm test`.
import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { fileURLToPath } from "node:url";
import { test } from "node:test";

const bin = fileURLToPath(new URL("../bin/tool", import.meta.url));
const run = (...args: string[]) => {
  const r = spawnSync(bin, args, { encoding: "utf8", stdio: ["ignore", "pipe", "pipe"] });
  return { out: r.stdout, err: r.stderr, code: r.status };
};

test("help goes to stdout and exits 0, before any action", () => {
  for (const args of [["--help"], ["-h"], ["help"], ["greet", "--help"], ["greet", "-h"]]) {
    const r = run(...args);
    assert.equal(r.code, 0, args.join(" "));
    assert.notEqual(r.out, "", args.join(" "));
    assert.equal(r.err, "", args.join(" "));
  }
});

test("usage errors exit 2 with a short message on stderr", () => {
  for (const args of [["--zz"], ["nope"], ["greet", "Ada", "--zz"], ["greet"]]) {
    const r = run(...args);
    assert.equal(r.code, 2, args.join(" "));
    assert.equal(r.out, "", args.join(" "));
    assert.match(r.err, /--help/, args.join(" "));
  }
});

test("--version prints a version", () => {
  assert.match(run("--version").out, /\d+\.\d+/);
});

test("--json is one document, failures included", () => {
  assert.deepEqual(JSON.parse(run("greet", "Ada", "--json").out), { greeting: "hello, Ada" });
  const r = run("greet", "--json");
  assert.equal(r.code, 2);
  assert.equal(JSON.parse(r.out).usage, true);
});

test("describe lists every command", () => {
  const d = JSON.parse(run("describe").out);
  assert.ok(d.commands.some((c: { name: string }) => c.name === "greet"));
});

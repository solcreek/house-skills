package main

// The rubric's must rules, as tests: build the binary once, run it. Tests
// run in parallel.

import (
	"bytes"
	"context"
	"encoding/json"
	"os"
	"os/exec"
	"path/filepath"
	"slices"
	"strings"
	"testing"
	"time"
)

var bin string

func TestMain(m *testing.M) {
	dir, _ := os.MkdirTemp("", "tool")
	bin = filepath.Join(dir, "tool")
	if out, err := exec.Command("go", "build", "-o", bin, ".").CombinedOutput(); err != nil {
		panic(string(out))
	}
	code := m.Run()
	os.RemoveAll(dir)
	os.Exit(code)
}

type result struct {
	out, err string
	code     int
}

// runIn runs in dir with stdin at EOF and a timeout, so a command that
// prompts fails instead of hanging; TOOL_HOME isolates state (conventions).
func runIn(t *testing.T, dir, home string, args ...string) result {
	t.Helper()
	ctx, cancel := context.WithTimeout(context.Background(), 10*time.Second)
	defer cancel()
	var o, e bytes.Buffer
	cmd := exec.CommandContext(ctx, bin, args...)
	cmd.Dir = dir
	cmd.Env = append(os.Environ(), "NO_COLOR=1", "TOOL_HOME="+home)
	cmd.Stdin = strings.NewReader("")
	cmd.Stdout, cmd.Stderr = &o, &e
	_ = cmd.Run()
	return result{o.String(), e.String(), cmd.ProcessState.ExitCode()}
}

func run(t *testing.T, dir string, args ...string) result {
	return runIn(t, dir, filepath.Join(dir, ".home"), args...)
}

func exists(path string) bool { _, err := os.Lstat(path); return err == nil }

func decode(t *testing.T, s string) map[string]any {
	t.Helper()
	var v map[string]any
	if err := json.Unmarshal([]byte(s), &v); err != nil {
		t.Fatalf("not JSON: %q", s)
	}
	return v
}

// tree is a directory with three files of different sizes.
func tree(t *testing.T) string {
	d := t.TempDir()
	must(t, os.Mkdir(filepath.Join(d, "sub"), 0o755))
	must(t, os.WriteFile(filepath.Join(d, "a"), bytes.Repeat([]byte("x"), 30), 0o644))
	must(t, os.WriteFile(filepath.Join(d, "b"), bytes.Repeat([]byte("x"), 20), 0o644))
	must(t, os.WriteFile(filepath.Join(d, "sub", "c"), bytes.Repeat([]byte("x"), 10), 0o644))
	return d
}

func must(t *testing.T, err error) {
	t.Helper()
	if err != nil {
		t.Fatal(err)
	}
}

func TestHelpGoesToStdout(t *testing.T) {
	t.Parallel()
	dir := t.TempDir()
	cases := [][]string{{}, {"--help"}, {"-h"}, {"help", "touch"}, {"files", "--help"}, {"touch", "-h"}, {"touch", "x", "--help"}, {"describe", "-h"}}
	for _, args := range cases {
		r := run(t, dir, args...)
		if r.code != 0 || !strings.Contains(r.out, "Usage") || r.err != "" {
			t.Errorf("%v: exit %d, stdout %q, stderr %q", args, r.code, r.out, r.err)
		}
	}
	if exists(filepath.Join(dir, "x")) {
		t.Error("touch x --help created x")
	}
	if r := run(t, dir, "files", "-h"); !strings.Contains(r.out, "3 partial") {
		t.Errorf("no exit codes in files -h: %q", r.out)
	}
}

func TestUsageErrorsExit2(t *testing.T) {
	t.Parallel()
	dir := t.TempDir()
	cases := [][]string{
		{"--zz"}, {"nope"}, {"-v"}, {"describe", "zz"},
		{"files", "a", "b"}, {"files", "--zz"}, {"files", "--limit", "x"}, {"files", "--limit=-1"}, {"files", "A\x07"},
		// A mutating command gets --dry-run, or the missing-flag check would answer first.
		{"touch", "--dry-run"}, {"touch", "a", "b", "--dry-run"}, {"touch", "", "--dry-run"}, {"touch", "x", "--dry-run", "--zz"},
	}
	for _, args := range cases {
		r := run(t, dir, args...)
		if r.code != exitUsage || r.out != "" || !strings.Contains(r.err, "--help") {
			t.Errorf("%v: exit %d, stdout %q, stderr %q", args, r.code, r.out, r.err)
		}
	}
}

func TestJSONBeforeOrAfterAndOnFailure(t *testing.T) {
	t.Parallel()
	dir := tree(t)
	after, before := run(t, dir, "files", "--json"), run(t, dir, "--json", "files")
	if after.out != before.out {
		t.Errorf("--json before and after differ: %q, %q", before.out, after.out)
	}
	for _, args := range [][]string{{"files", "--json", "--zz"}, {"files", "--zz", "--json"}, {"touch", "x", "--json"}} {
		r := run(t, dir, args...)
		if r.code != exitUsage || decode(t, r.out)["usage"] != true {
			t.Errorf("%v: exit %d, stdout %q", args, r.code, r.out)
		}
	}
}

func TestAListIsCapped(t *testing.T) {
	t.Parallel()
	dir := tree(t)
	r := run(t, dir, "files", "--limit", "2", "--json")
	v := decode(t, r.out)
	if v["total"] != 3.0 || v["shown"] != 2.0 || v["truncated"] != true || !strings.Contains(r.err, "2 of 3 shown; --limit 0 for all") {
		t.Errorf("capped: %q, stderr %q", r.out, r.err)
	}
	items := v["items"].([]any)
	if items[0].(map[string]any)["path"] != "a" || items[1].(map[string]any)["path"] != "b" {
		t.Errorf("order: %v", items)
	}
	if all := run(t, dir, "files", "--limit", "0", "--json"); decode(t, all.out)["truncated"] != false || all.err != "" {
		t.Errorf("--limit 0: %q, stderr %q", all.out, all.err)
	}
	if text := run(t, dir, "files", "--limit", "1"); text.out != "30\ta\n" {
		t.Errorf("text: %q", text.out)
	}
}

func TestMissingInputAndPartial(t *testing.T) {
	t.Parallel()
	dir := tree(t)
	r := run(t, dir, "files", "nowhere", "--json")
	if e, _ := decode(t, r.out)["error"].(string); r.code != exitFailed || !strings.Contains(e, "nowhere") {
		t.Errorf("missing: exit %d, %q", r.code, r.out)
	}
	if os.Geteuid() == 0 {
		return // root reads everything
	}
	sub := filepath.Join(dir, "sub")
	must(t, os.Chmod(sub, 0o000))
	r = run(t, dir, "files", "--json")
	must(t, os.Chmod(sub, 0o755))
	v := decode(t, r.out)
	problems, _ := v["problems"].([]any)
	if r.code != exitPartial || len(problems) != 1 || problems[0].(map[string]any)["input"] != "sub" || v["total"] != 2.0 {
		t.Errorf("partial: exit %d, %q", r.code, r.out)
	}
}

func TestMutatingNeedsExactlyOneOfDryRunAndYes(t *testing.T) {
	t.Parallel()
	dir := t.TempDir()
	for _, flags := range [][]string{{}, {"--dry-run", "--yes"}} {
		r := run(t, dir, append([]string{"touch", "x"}, flags...)...)
		if r.code != exitUsage || !strings.Contains(r.err, "--dry-run") {
			t.Errorf("%v: exit %d, stderr %q", flags, r.code, r.err)
		}
	}
	if exists(filepath.Join(dir, "x")) {
		t.Error("created x without --yes")
	}
}

func TestDryRunYesStateRerun(t *testing.T) {
	t.Parallel()
	dir, home := t.TempDir(), t.TempDir()
	v := decode(t, runIn(t, dir, home, "touch", "x", "--dry-run", "--json").out)
	if v["changed"] != true || v["dryRun"] != true || exists(filepath.Join(dir, "x")) || exists(filepath.Join(home, "state")) {
		t.Errorf("dry run: %v", v)
	}
	v = decode(t, runIn(t, dir, home, "touch", "x", "--yes", "--json").out)
	if v["changed"] != true || !exists(filepath.Join(dir, "x")) {
		t.Errorf("--yes: %v", v)
	}
	var history []string
	b, _ := os.ReadFile(filepath.Join(home, "state", "history.json"))
	if json.Unmarshal(b, &history) != nil || !slices.Equal(history, []string{v["path"].(string)}) {
		t.Errorf("history: %q", b)
	}
	r := runIn(t, dir, home, "touch", "x", "--yes", "--json")
	if r.code != 0 || decode(t, r.out)["changed"] != false {
		t.Errorf("rerun: exit %d, %q", r.code, r.out)
	}
}

func TestOutputPathCannotEscapeOrFollowASymlink(t *testing.T) {
	t.Parallel()
	outside := t.TempDir()
	must(t, os.WriteFile(filepath.Join(outside, "keep"), []byte("precious"), 0o644))
	dir, _ := filepath.EvalSymlinks(t.TempDir())
	must(t, os.Mkdir(filepath.Join(dir, "sub"), 0o755))
	must(t, os.Symlink(outside, filepath.Join(dir, "out")))
	must(t, os.Symlink(filepath.Join(outside, "keep"), filepath.Join(dir, "link")))
	must(t, os.Symlink(filepath.Join(outside, "gone"), filepath.Join(dir, "dangling")))
	must(t, os.Symlink(outside, filepath.Join(dir, "sub", "up")))
	paths := []string{"../x", "/tmp/x", "out/x", "link", "dangling", "sub/up/../x", "sub/", dir + "/sub/", ".", dir, "newdir/"}
	for _, path := range paths {
		for _, mode := range []string{"--dry-run", "--yes"} {
			r := run(t, dir, "touch", path, mode, "--json")
			if r.code != exitFailed || decode(t, r.out)["usage"] != false {
				t.Errorf("%s %s: exit %d, %q", path, mode, r.code, r.out)
			}
		}
	}
	if b, _ := os.ReadFile(filepath.Join(outside, "keep")); string(b) != "precious" {
		t.Error("overwrote a file outside")
	}
	for _, p := range []string{filepath.Join(outside, "gone"), filepath.Join(outside, "x"), filepath.Join(dir, "sub", "sub"), filepath.Join(dir, "newdir")} {
		if exists(p) {
			t.Errorf("created %s", p)
		}
	}
}

func TestAbsolutePathInsideIsFine(t *testing.T) {
	t.Parallel()
	dir, _ := filepath.EvalSymlinks(t.TempDir())
	if r := run(t, dir, "touch", filepath.Join(dir, "x"), "--yes"); r.code != 0 {
		t.Errorf("exit %d: %s", r.code, r.err)
	}
}

func TestAFailedWriteNamesTheFile(t *testing.T) {
	t.Parallel()
	if os.Geteuid() == 0 {
		return // root writes anywhere
	}
	dir := t.TempDir()
	ro := filepath.Join(dir, "ro")
	must(t, os.Mkdir(ro, 0o500))
	defer os.Chmod(ro, 0o700)
	r := run(t, dir, "touch", "ro/x", "--yes", "--json")
	if e, _ := decode(t, r.out)["error"].(string); r.code != exitFailed || !strings.Contains(e, "cannot create") {
		t.Errorf("exit %d, %q", r.code, r.out)
	}
}

func TestDescribe(t *testing.T) {
	t.Parallel()
	r := run(t, t.TempDir(), "describe")
	type flag struct {
		Name    string
		Type    string
		Default any
	}
	type arg struct {
		Name     string
		Required bool
		Default  any
	}
	var d struct {
		ExitCodes map[string]int
		Flags     []flag
		Commands  []struct {
			Name     string
			Mutates  bool
			Args     []arg
			Flags    []flag
			Examples []string
		}
	}
	if r.code != 0 || json.Unmarshal([]byte(r.out), &d) != nil {
		t.Fatalf("exit %d, stdout %q", r.code, r.out)
	}
	names := func(fs []flag) (n []string) {
		for _, f := range fs {
			n = append(n, f.Name)
		}
		slices.Sort(n)
		return n
	}
	if d.ExitCodes["partial"] != exitPartial || !slices.Equal(names(d.Flags), []string{"help", "json", "version"}) {
		t.Errorf("root: exit codes %v, flags %v", d.ExitCodes, names(d.Flags))
	}
	var keys map[string]any
	_ = json.Unmarshal([]byte(r.out), &keys)
	if _, ok := keys["usage"]; ok {
		t.Error("the top level has a usage line")
	}
	want := map[string][]string{"files": {"help", "json", "limit"}, "touch": {"dry-run", "help", "json", "yes"}}
	for _, c := range d.Commands {
		if w, ok := want[c.Name]; ok {
			if !slices.Equal(names(c.Flags), w) || c.Mutates != (c.Name == "touch") || len(c.Examples) == 0 {
				t.Errorf("%s: flags %v, mutates %v, examples %v", c.Name, names(c.Flags), c.Mutates, c.Examples)
			}
			delete(want, c.Name)
		}
		if c.Name == "files" {
			if len(c.Args) != 1 || c.Args[0] != (arg{"DIR", false, "."}) {
				t.Errorf("files args: %+v", c.Args)
			}
			for _, f := range c.Flags {
				if f.Name == "limit" && (f.Type != "integer" || f.Default != "20") {
					t.Errorf("limit: %+v", f)
				}
			}
		}
	}
	if len(want) > 0 {
		t.Errorf("missing commands %v", want)
	}
}

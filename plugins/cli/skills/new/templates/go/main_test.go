package main

// The rubric's must rules, as tests: build the binary once, run it.

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

// run in dir with stdin at EOF and a timeout, so a command that prompts fails
// instead of hanging; TOOL_HOME isolates any state (conventions).
func run(t *testing.T, dir string, args ...string) (stdout, stderr string, code int) {
	t.Helper()
	ctx, cancel := context.WithTimeout(context.Background(), 10*time.Second)
	defer cancel()
	var o, e bytes.Buffer
	cmd := exec.CommandContext(ctx, bin, args...)
	cmd.Dir = dir
	cmd.Env = append(os.Environ(), "NO_COLOR=1", "TOOL_HOME="+filepath.Join(dir, ".home"))
	cmd.Stdin = strings.NewReader("")
	cmd.Stdout, cmd.Stderr = &o, &e
	_ = cmd.Run()
	return o.String(), e.String(), cmd.ProcessState.ExitCode()
}

func exists(path string) bool { _, err := os.Stat(path); return err == nil }

func decode(t *testing.T, s string) map[string]any {
	t.Helper()
	var v map[string]any
	if err := json.Unmarshal([]byte(s), &v); err != nil {
		t.Fatalf("not JSON: %q", s)
	}
	return v
}

func TestHelpGoesToStdout(t *testing.T) {
	dir := t.TempDir()
	for _, args := range [][]string{{"--help"}, {"-h"}, {"help", "touch"}, {"greet", "--help"}, {"touch", "-h"}, {"touch", "x", "--help"}} {
		out, errOut, code := run(t, dir, args...)
		if code != 0 || !strings.Contains(out, "Usage") || errOut != "" {
			t.Errorf("%v: exit %d, stdout %q, stderr %q", args, code, out, errOut)
		}
	}
	if exists(filepath.Join(dir, "x")) {
		t.Error("touch x --help created x")
	}
}

func TestUsageErrorsExit2(t *testing.T) {
	dir := t.TempDir()
	cases := [][]string{
		{"--zz"}, {"nope"}, {"greet"}, {"greet", "Ada", "Bob"}, {"greet", "Ada", "--zz"},
		{"describe", "zz"}, {"greet", "A\x07da"}, {"touch", "", "--yes"}, {"-v"},
	}
	for _, args := range cases {
		out, errOut, code := run(t, dir, args...)
		if code != exitUsage || out != "" || !strings.Contains(errOut, "--help") {
			t.Errorf("%v: exit %d, stdout %q, stderr %q", args, code, out, errOut)
		}
	}
}

func TestJSONFailureWhateverTheOrder(t *testing.T) {
	dir := t.TempDir()
	for _, args := range [][]string{{"greet", "--json", "--zz"}, {"greet", "--zz", "--json"}} {
		out, _, code := run(t, dir, args...)
		if code != exitUsage || decode(t, out)["usage"] != true {
			t.Errorf("%v: exit %d, stdout %q", args, code, out)
		}
	}
}

func TestMutatingNeedsExactlyOneOfDryRunAndYes(t *testing.T) {
	dir := t.TempDir()
	for _, flags := range [][]string{{}, {"--dry-run", "--yes"}} {
		_, errOut, code := run(t, dir, append([]string{"touch", "x"}, flags...)...)
		if code != exitUsage || !strings.Contains(errOut, "--dry-run") {
			t.Errorf("%v: exit %d, stderr %q", flags, code, errOut)
		}
	}
	if exists(filepath.Join(dir, "x")) {
		t.Error("created x without --yes")
	}
}

func TestDryRunYesRerun(t *testing.T) {
	dir := t.TempDir()
	out, _, _ := run(t, dir, "touch", "x", "--dry-run", "--json")
	if v := decode(t, out); v["changed"] != true || v["dryRun"] != true || exists(filepath.Join(dir, "x")) {
		t.Errorf("dry run: %q", out)
	}
	out, _, _ = run(t, dir, "touch", "x", "--yes", "--json")
	if v := decode(t, out); v["changed"] != true || !exists(filepath.Join(dir, "x")) {
		t.Errorf("--yes: %q", out)
	}
	out, _, code := run(t, dir, "touch", "x", "--yes", "--json")
	if v := decode(t, out); code != 0 || v["changed"] != false {
		t.Errorf("rerun: exit %d, %q", code, out)
	}
}

func TestOutputPathCannotEscapeOrFollowASymlink(t *testing.T) {
	outside := t.TempDir()
	if err := os.WriteFile(filepath.Join(outside, "keep"), []byte("precious"), 0o644); err != nil {
		t.Fatal(err)
	}
	dir, _ := filepath.EvalSymlinks(t.TempDir())
	must := func(err error) {
		if err != nil {
			t.Fatal(err)
		}
	}
	must(os.Mkdir(filepath.Join(dir, "sub"), 0o755))
	must(os.Symlink(outside, filepath.Join(dir, "out")))
	must(os.Symlink(filepath.Join(outside, "keep"), filepath.Join(dir, "link")))
	must(os.Symlink(filepath.Join(outside, "gone"), filepath.Join(dir, "dangling")))
	must(os.Symlink(outside, filepath.Join(dir, "sub", "up")))
	paths := []string{"../x", "/tmp/x", "out/x", "link", "dangling", "sub/up/../x", "sub/", dir + "/sub/", ".", dir, "newdir/"}
	for _, path := range paths {
		for _, mode := range []string{"--dry-run", "--yes"} {
			out, _, code := run(t, dir, "touch", path, mode, "--json")
			if code != exitFailed || decode(t, out)["usage"] != false {
				t.Errorf("%s %s: exit %d, %q", path, mode, code, out)
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
	dir, _ := filepath.EvalSymlinks(t.TempDir())
	if _, errOut, code := run(t, dir, "touch", filepath.Join(dir, "x"), "--yes"); code != 0 {
		t.Errorf("exit %d: %s", code, errOut)
	}
}

func TestAFailedWriteNamesTheFile(t *testing.T) {
	dir := t.TempDir()
	ro := filepath.Join(dir, "ro")
	if err := os.Mkdir(ro, 0o500); err != nil {
		t.Fatal(err)
	}
	defer os.Chmod(ro, 0o700)
	out, _, code := run(t, dir, "touch", "ro/x", "--yes", "--json")
	if e, _ := decode(t, out)["error"].(string); code != exitFailed || !strings.Contains(e, "cannot create") {
		t.Errorf("exit %d, %q", code, out)
	}
}

func TestDescribeListsEveryCommand(t *testing.T) {
	out, _, code := run(t, t.TempDir(), "describe")
	type flag struct{ Name string }
	var d struct {
		ExitCodes map[string]int
		Flags     []flag
		Commands  []struct {
			Name    string
			Mutates bool
			Flags   []flag
		}
	}
	if code != 0 || json.Unmarshal([]byte(out), &d) != nil {
		t.Fatalf("exit %d, stdout %q", code, out)
	}
	names := func(fs []flag) (n []string) {
		for _, f := range fs {
			n = append(n, f.Name)
		}
		slices.Sort(n)
		return n
	}
	if d.ExitCodes["usage"] != exitUsage || !slices.Equal(names(d.Flags), []string{"help", "json", "version"}) {
		t.Errorf("root: exit codes %v, flags %v", d.ExitCodes, names(d.Flags))
	}
	var keys map[string]any
	_ = json.Unmarshal([]byte(out), &keys)
	if _, ok := keys["usage"]; ok {
		t.Error("the top level has a usage line")
	}
	want := map[string][]string{"greet": {"help", "json"}, "touch": {"dry-run", "help", "json", "yes"}}
	for _, c := range d.Commands {
		if w, ok := want[c.Name]; ok {
			if !slices.Equal(names(c.Flags), w) || c.Mutates != (c.Name == "touch") {
				t.Errorf("%s: flags %v, mutates %v", c.Name, names(c.Flags), c.Mutates)
			}
			delete(want, c.Name)
		}
	}
	if len(want) > 0 {
		t.Errorf("missing commands %v", want)
	}
}

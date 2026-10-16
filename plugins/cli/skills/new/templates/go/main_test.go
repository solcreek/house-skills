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
	for _, args := range [][]string{{"--help"}, {"-h"}, {"greet", "--help"}, {"touch", "-h"}, {"touch", "x", "--help"}} {
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
	cases := [][]string{{"--zz"}, {"nope"}, {"greet"}, {"greet", "Ada", "Bob"}, {"greet", "Ada", "--zz"}, {"describe", "zz"}, {"greet", "A\x07da"}}
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

func TestOutputPathCannotEscape(t *testing.T) {
	dir := t.TempDir()
	if err := os.Symlink(os.TempDir(), filepath.Join(dir, "out")); err != nil {
		t.Fatal(err)
	}
	for _, path := range []string{"../x", "/tmp/x", "out/x"} {
		out, _, code := run(t, dir, "touch", path, "--yes", "--json")
		if e, _ := decode(t, out)["error"].(string); code != exitFailed || !strings.Contains(e, "outside") {
			t.Errorf("%s: exit %d, %q", path, code, out)
		}
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
	if d.ExitCodes["usage"] != exitUsage || !slices.Contains(names(d.Flags), "version") {
		t.Errorf("root: exit codes %v, flags %v", d.ExitCodes, names(d.Flags))
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

package main

// The rubric's must rules, as tests: build the binary once, run it.

import (
	"bytes"
	"encoding/json"
	"os"
	"os/exec"
	"path/filepath"
	"slices"
	"testing"
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

func run(args ...string) (stdout, stderr string, code int) {
	var o, e bytes.Buffer
	cmd := exec.Command(bin, args...)
	cmd.Stdout, cmd.Stderr = &o, &e
	_ = cmd.Run()
	return o.String(), e.String(), cmd.ProcessState.ExitCode()
}

func TestHelpGoesToStdout(t *testing.T) {
	for _, args := range [][]string{{"--help"}, {"-h"}, {"greet", "--help"}} {
		out, errOut, code := run(args...)
		if code != 0 || out == "" || errOut != "" {
			t.Errorf("%v: exit %d, stdout %q, stderr %q", args, code, out, errOut)
		}
	}
}

func TestJSONFailureWhateverTheOrder(t *testing.T) {
	for _, args := range [][]string{{"greet", "--json", "--zz"}, {"greet", "--zz", "--json"}} {
		out, _, code := run(args...)
		var v struct{ Usage bool }
		if code != exitUsage || json.Unmarshal([]byte(out), &v) != nil || !v.Usage {
			t.Errorf("%v: exit %d, stdout %q", args, code, out)
		}
	}
}

func TestUsageErrorsExit2(t *testing.T) {
	for _, args := range [][]string{{"--zz"}, {"greet", "Ada", "--zz"}, {"greet"}, {"nope"}} {
		out, errOut, code := run(args...)
		if code != exitUsage || out != "" || errOut == "" {
			t.Errorf("%v: exit %d, stdout %q, stderr %q", args, code, out, errOut)
		}
	}
}

func TestDescribeListsEveryCommand(t *testing.T) {
	out, _, code := run("describe")
	type flag struct{ Name string }
	var d struct {
		ExitCodes map[string]int
		Flags     []flag
		Commands  []struct {
			Name  string
			Flags []flag
		}
	}
	if code != 0 || json.Unmarshal([]byte(out), &d) != nil {
		t.Fatalf("exit %d, stdout %q", code, out)
	}
	names := func(fs []flag) (n []string) {
		for _, f := range fs {
			n = append(n, f.Name)
		}
		return n
	}
	if d.ExitCodes["usage"] != exitUsage || !slices.Contains(names(d.Flags), "version") {
		t.Errorf("root: exit codes %v, flags %v", d.ExitCodes, names(d.Flags))
	}
	for _, c := range d.Commands {
		if c.Name == "greet" {
			if !slices.Contains(names(c.Flags), "json") || !slices.Contains(names(c.Flags), "help") {
				t.Errorf("greet flags %v, want json and help", names(c.Flags))
			}
			return
		}
	}
	t.Errorf("no greet in %q", out)
}

func TestJSONIsOneDocument(t *testing.T) {
	out, _, code := run("greet", "Ada", "--json")
	var v map[string]string
	if code != 0 || json.Unmarshal([]byte(out), &v) != nil || v["greeting"] != "hello, Ada" {
		t.Errorf("exit %d, stdout %q", code, out)
	}
}

package main

// The rubric's must rules, as tests: build the binary once, run it.

import (
	"bytes"
	"encoding/json"
	"os"
	"os/exec"
	"path/filepath"
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
	var d struct{ Commands []struct{ Name string } }
	if code != 0 || json.Unmarshal([]byte(out), &d) != nil {
		t.Fatalf("exit %d, stdout %q", code, out)
	}
	for _, c := range d.Commands {
		if c.Name == "greet" {
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

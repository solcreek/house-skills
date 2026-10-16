// Command tool: one line on what it is for.
//
// Conventions: ../audit/references/conventions.md in the cli plugin.
package main

import (
	"encoding/json"
	"errors"
	"fmt"
	"os"
	"path/filepath"
	"slices"
	"strings"
	"unicode"

	"github.com/spf13/cobra"
	"github.com/spf13/pflag"
)

var version = "0.1.0" // set by -ldflags "-X main.version=..." at release

// Every exit code, in help and in `describe`. Add to it; never reuse one.
const (
	exitOK     = 0
	exitFailed = 1
	exitUsage  = 2
)

// usageError marks a mistake in the invocation: exit 2, a short message.
type usageError struct{ error }

var asJSON bool

func newRoot() *cobra.Command {
	root := &cobra.Command{
		Use:     "tool",
		Short:   "One line on what it is for",
		Long:    "One line on what it is for.\n\nExit codes: 0 ok, 1 failed, 2 usage.\nRun 'tool describe' for every command as JSON.",
		Version: version,
		// cobra prints the whole usage on every error, and exits 1 for a bad
		// flag; main prints a short message and exits 2 instead (rubric D3, E3).
		SilenceUsage:  true,
		SilenceErrors: true,
		// Without a RunE, cobra answers an unknown subcommand with help and
		// exit 0. This makes it an error (rubric S2).
		Args: usageArgs(cobra.NoArgs),
		RunE: func(cmd *cobra.Command, _ []string) error { return cmd.Help() },
	}
	root.SetVersionTemplate("tool {{.Version}}\n")
	root.PersistentFlags().BoolVar(&asJSON, "json", false, "print one JSON document on stdout")
	root.SetFlagErrorFunc(func(_ *cobra.Command, err error) error { return usageError{err} })

	root.AddCommand(&cobra.Command{
		Use:     "greet NAME",
		Short:   "Print a greeting",
		Example: "  tool greet Ada\n  tool greet Ada --json",
		Args:    usageArgs(cobra.ExactArgs(1)),
		RunE: func(cmd *cobra.Command, args []string) error {
			name, err := plain("NAME", args[0])
			if err != nil {
				return err
			}
			greeting := "hello, " + name
			return output(cmd, map[string]any{"greeting": greeting}, greeting)
		},
	})

	touch := &cobra.Command{
		Use:     "touch PATH",
		Short:   "Create an empty file under the current directory (changes things)",
		Example: "  tool touch notes.txt --dry-run\n  tool touch notes.txt --yes --json",
		Args:    usageArgs(cobra.ExactArgs(1)),
		RunE: func(cmd *cobra.Command, args []string) error {
			dryRun, err := change(cmd)
			if err != nil {
				return err
			}
			path, err := plain("PATH", args[0])
			if err != nil {
				return err
			}
			cwd, err := os.Getwd()
			if err != nil {
				return err
			}
			target, err := inside(cwd, path)
			if err != nil {
				return err
			}
			// Already there is success with nothing changed (rubric S5).
			_, statErr := os.Stat(target)
			changed := errors.Is(statErr, os.ErrNotExist)
			if changed && !dryRun {
				f, err := os.OpenFile(target, os.O_WRONLY|os.O_CREATE|os.O_EXCL, 0o644)
				if err != nil {
					return err
				}
				f.Close()
			}
			verb := map[bool]string{true: "created", false: "exists"}[changed]
			if changed && dryRun {
				verb = "would create"
			}
			return output(cmd, map[string]any{"path": target, "changed": changed, "dryRun": dryRun}, verb+" "+target)
		},
	}
	mutating(touch)
	root.AddCommand(touch)

	root.AddCommand(&cobra.Command{
		Use:   "describe",
		Short: "Print every command, flag and exit code as JSON",
		Args:  usageArgs(cobra.NoArgs),
		RunE:  func(*cobra.Command, []string) error { return printJSON(describe(root)) },
	})
	return root
}

// mutating gives a command --dry-run and --yes; its RunE calls change first.
func mutating(c *cobra.Command) {
	c.Flags().Bool("dry-run", false, "show what would change; change nothing")
	c.Flags().Bool("yes", false, "apply the change")
	c.Annotations = map[string]string{"mutates": "true"}
}

// change reports whether this is a dry run. Neither flag, or both, is a
// usage error: a mutating command never acts or prompts by default.
func change(cmd *cobra.Command) (dryRun bool, err error) {
	dryRun, _ = cmd.Flags().GetBool("dry-run")
	yes, _ := cmd.Flags().GetBool("yes")
	if dryRun == yes {
		return false, usageError{fmt.Errorf("%s changes things: pass --dry-run to preview, or --yes to apply (one of them)", cmd.Name())}
	}
	return dryRun, nil
}

// plain refuses control characters in an argument (rubric A9).
func plain(what, s string) (string, error) {
	if strings.IndexFunc(s, unicode.IsControl) >= 0 {
		return "", usageError{fmt.Errorf("%s contains a control character", what)}
	}
	return s, nil
}

// inside resolves an output path and refuses one that lands outside root
// once symlinks are resolved (rubric A9).
func inside(root, path string) (string, error) {
	realRoot, err := filepath.EvalSymlinks(root)
	if err != nil {
		return "", err
	}
	if !filepath.IsAbs(path) {
		path = filepath.Join(root, path)
	}
	parent, err := filepath.EvalSymlinks(filepath.Dir(path))
	if err != nil {
		return "", fmt.Errorf("no such directory: %s", filepath.Dir(path))
	}
	target := filepath.Join(parent, filepath.Base(path))
	rel, err := filepath.Rel(realRoot, target)
	if err != nil || rel == "." || rel == ".." || strings.HasPrefix(rel, ".."+string(filepath.Separator)) {
		return "", fmt.Errorf("%s is outside %s; tool writes only under the current directory", path, realRoot)
	}
	return target, nil
}

// usageArgs makes an argument-count error a usage error.
func usageArgs(check cobra.PositionalArgs) cobra.PositionalArgs {
	return func(cmd *cobra.Command, args []string) error {
		if err := check(cmd, args); err != nil {
			return usageError{err}
		}
		return nil
	}
}

func output(cmd *cobra.Command, v any, text string) error {
	if asJSON {
		return printJSON(v)
	}
	fmt.Fprintln(cmd.OutOrStdout(), text)
	return nil
}

func printJSON(v any) error {
	enc := json.NewEncoder(os.Stdout)
	enc.SetIndent("", "  ")
	return enc.Encode(v)
}

// describe is the same definitions as --help, as data (rubric A10), in the
// shape every template's describe prints: see references/describe.md.
func describe(c *cobra.Command) map[string]any {
	// cobra adds --help and --version only when a command runs; add them now
	// so describe lists what --help shows.
	c.InitDefaultHelpFlag()
	if !c.HasParent() {
		c.InitDefaultVersionFlag()
	}
	flags := []map[string]any{}
	add := func(f *pflag.Flag) {
		if f.Hidden {
			return
		}
		var short, def any
		if f.Shorthand != "" {
			short = f.Shorthand
		}
		takesValue := f.Value.Type() != "bool"
		if takesValue && f.DefValue != "" {
			def = f.DefValue
		}
		flags = append(flags, map[string]any{
			"name": f.Name, "short": short, "takesValue": takesValue, "default": def, "description": f.Usage,
		})
	}
	c.LocalFlags().VisitAll(add)
	c.InheritedFlags().VisitAll(add)
	subs := []map[string]any{}
	for _, s := range c.Commands() {
		if s.IsAvailableCommand() && s.Name() != "completion" {
			subs = append(subs, describe(s))
		}
	}
	d := map[string]any{
		"name": c.Name(), "summary": c.Short, "usage": c.UseLine(), "flags": flags, "commands": subs,
		"mutates": c.Annotations["mutates"] == "true",
	}
	if !c.HasParent() {
		d["version"] = version
		d["exitCodes"] = map[string]int{"ok": exitOK, "failed": exitFailed, "usage": exitUsage}
	}
	return d
}

func main() {
	root := newRoot()
	root.SetOut(os.Stdout) // help on stdout (rubric E1)
	root.SetErr(os.Stderr)
	err := root.Execute()
	if err == nil {
		os.Exit(exitOK)
	}
	fmt.Fprintf(os.Stderr, "tool: %v\n", err)
	var ue usageError
	usage := errors.As(err, &ue)
	if usage {
		fmt.Fprintln(os.Stderr, "Run 'tool --help' for usage.")
	}
	// Under --json a failure is JSON too (rubric A5). Parsing may have
	// stopped before it saw --json, so look at the arguments themselves.
	if asJSON || slices.Contains(os.Args[1:], "--json") {
		_ = printJSON(map[string]any{"error": err.Error(), "usage": usage})
	}
	if usage {
		os.Exit(exitUsage)
	}
	os.Exit(exitFailed)
}

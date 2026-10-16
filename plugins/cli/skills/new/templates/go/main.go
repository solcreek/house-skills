// Command tool: one line on what it is for.
package main

import (
	"encoding/json"
	"errors"
	"fmt"
	"os"
	"slices"

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
			greeting := "hello, " + args[0]
			if asJSON {
				return printJSON(map[string]string{"greeting": greeting})
			}
			fmt.Fprintln(cmd.OutOrStdout(), greeting)
			return nil
		},
	})
	root.AddCommand(&cobra.Command{
		Use:   "describe",
		Short: "Print every command, flag and exit code as JSON",
		Args:  usageArgs(cobra.NoArgs),
		RunE:  func(*cobra.Command, []string) error { return printJSON(describe(root)) },
	})
	return root
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
		var short any
		if f.Shorthand != "" {
			short = f.Shorthand
		}
		flags = append(flags, map[string]any{
			"name": f.Name, "short": short, "takesValue": f.Value.Type() != "bool", "description": f.Usage,
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
	d := map[string]any{"name": c.Name(), "summary": c.Short, "usage": c.UseLine(), "flags": flags, "commands": subs}
	if !c.HasParent() {
		d["version"] = version
		d["exitCodes"] = map[string]int{"ok": exitOK, "failed": exitFailed, "usage": exitUsage}
	}
	return d
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

func printJSON(v any) error {
	enc := json.NewEncoder(os.Stdout)
	enc.SetIndent("", "  ")
	return enc.Encode(v)
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

// Command tool: one line on what it is for.
//
// Conventions: ../audit/references/conventions.md in the cli plugin.
package main

import (
	"encoding/json"
	"errors"
	"fmt"
	"io/fs"
	"os"
	"path/filepath"
	"slices"
	"sort"
	"strconv"
	"strings"
	"unicode"

	"github.com/spf13/cobra"
	"github.com/spf13/pflag"
)

var version = "0.1.0" // set by -ldflags "-X main.version=..." at release

// Every exit code, in help and in `describe`. Add to it; never reuse one.
const (
	exitOK      = 0
	exitFailed  = 1
	exitUsage   = 2
	exitPartial = 3
)

const exitHelp = "Exit codes: 0 ok, 1 failed, 2 usage, 3 partial"

// usageError marks a mistake in the invocation: exit 2, a short message.
type usageError struct{ error }

// partialError carries a result that is already printed: exit 3.
type partialError struct{ error }

var asJSON bool

func newRoot() *cobra.Command {
	root := &cobra.Command{
		Use:     "tool",
		Short:   "One line on what it is for",
		Version: version,
		// cobra prints the whole usage on every error, and exits 1 for a bad
		// flag; main prints a short message and exits 2 instead (rubric D3, E3).
		SilenceUsage:  true,
		SilenceErrors: true,
		// Without a RunE, cobra answers an unknown subcommand with help and
		// exit 0. This makes it an error (rubric S2).
		Args: usageArgs(cobra.NoArgs),
		RunE: func(cmd *cobra.Command, _ []string) error { return cmd.Help() },
		// Every mutating command is guarded here, so none can forget to be.
		// cobra answers --help before this runs.
		PersistentPreRunE: func(cmd *cobra.Command, _ []string) error {
			if cmd.Annotations["mutates"] == "true" {
				_, err := change(cmd)
				return err
			}
			return nil
		},
	}
	root.SetVersionTemplate("tool {{.Version}}\n")
	// Our own --version, so cobra adds no -v: elsewhere -v means verbose.
	root.Flags().Bool("version", false, "print the version")
	root.PersistentFlags().BoolVar(&asJSON, "json", false, "print one JSON document on stdout")
	root.SetFlagErrorFunc(func(_ *cobra.Command, err error) error { return usageError{err} })
	// Exit codes on every help page (rubric E4); children inherit it.
	root.SetHelpTemplate(root.HelpTemplate() + "\n" + exitHelp + "\nRun 'tool describe' for every command as JSON.\n")

	files := &cobra.Command{
		Use:     "files [DIR]",
		Short:   "List the files under a directory, largest first",
		Long:    "List the files under a directory, largest first.\nDIR defaults to \".\".",
		Example: "  tool files\n  tool files src --limit 5 --json",
		// The default of an optional positional, for help and describe.
		Annotations: map[string]string{"default.DIR": "."},
		Args:        usageArgs(cobra.MaximumNArgs(1)),
		RunE: func(cmd *cobra.Command, args []string) error {
			dir := "."
			if len(args) == 1 {
				dir = args[0]
			}
			if _, err := plain("DIR", dir); err != nil {
				return err
			}
			// Lexically, like the other templates: `a/..` is the current directory.
			given, err := filepath.Abs(dir)
			if err != nil {
				return err
			}
			switch fi, err := os.Stat(given); {
			case errors.Is(err, os.ErrNotExist):
				return fmt.Errorf("no such directory: %s", given)
			case err != nil:
				return fmt.Errorf("cannot read %s: %s", given, cause(err))
			case !fi.IsDir():
				return fmt.Errorf("not a directory: %s", given)
			}
			// Walk the real path: WalkDir does not follow a symlinked root, and
			// paths then compare with the tool's own homes.
			root, err := filepath.EvalSymlinks(given)
			if err != nil {
				return fmt.Errorf("cannot read %s: %s", given, cause(err))
			}
			// The root itself unreadable is a failure, not a partial result.
			if _, err := os.ReadDir(root); err != nil {
				return fmt.Errorf("cannot read %s: %s", given, cause(err))
			}
			own := homes()
			limit := cmd.Flags().Lookup("limit").Value.(*wholeNumber).n
			type file struct {
				Path  string `json:"path"`
				Bytes int64  `json:"bytes"`
			}
			var found []file
			problems := []map[string]string{}
			// WalkDir does not follow symlinks. What cannot be read is a
			// problem, never silently "nothing there" (rubric A7).
			_ = filepath.WalkDir(root, func(p string, d fs.DirEntry, err error) error {
				rel, _ := filepath.Rel(root, p)
				if err != nil {
					problems = append(problems, map[string]string{"input": rel, "error": cause(err)})
					return nil
				}
				if d.IsDir() && p != root && own[p] {
					return filepath.SkipDir
				}
				if d.Type().IsRegular() {
					if info, err := d.Info(); err == nil {
						found = append(found, file{rel, info.Size()})
					} else {
						problems = append(problems, map[string]string{"input": rel, "error": err.Error()})
					}
				}
				return nil
			})
			sort.Slice(found, func(i, j int) bool {
				if found[i].Bytes != found[j].Bytes {
					return found[i].Bytes > found[j].Bytes
				}
				return found[i].Path < found[j].Path
			})
			items, list := capped(found, int(limit))
			if asJSON {
				doc := map[string]any{"root": root, "items": items}
				for k, v := range list {
					doc[k] = v
				}
				if len(problems) > 0 {
					doc["problems"] = problems
				}
				if err := printJSON(doc); err != nil {
					return err
				}
			} else {
				for _, f := range items {
					fmt.Fprintf(cmd.OutOrStdout(), "%d\t%s\n", f.Bytes, printable(f.Path))
				}
			}
			for _, p := range problems {
				fmt.Fprintf(os.Stderr, "tool: cannot read %s: %s\n", printable(p["input"]), p["error"])
			}
			if len(problems) > 0 {
				return partialError{fmt.Errorf("%d inputs could not be read", len(problems))}
			}
			return nil
		},
	}
	limit := wholeNumber{n: 20}
	files.Flags().Var(&limit, "limit", "show at most `N` items; 0 for all")
	root.AddCommand(files)

	touch := &cobra.Command{
		Use:     "touch PATH",
		Short:   "Create an empty file under the current directory",
		Long:    "Create an empty file under the current directory.\nChanges things: pass --dry-run to preview, --yes to apply.",
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
			_, statErr := os.Lstat(target)
			changed := errors.Is(statErr, os.ErrNotExist)
			if changed && !dryRun {
				f, err := os.OpenFile(target, os.O_WRONLY|os.O_CREATE|os.O_EXCL, 0o644)
				if err != nil {
					return fmt.Errorf("cannot create %s: %s", target, cause(err))
				}
				f.Close()
			}
			// State: what this tool created, kept across runs.
			recorded := changed && !dryRun && appendState("history.jsonl", map[string]string{"path": target})
			verb := map[bool]string{true: "created", false: "exists"}[changed]
			if changed && dryRun {
				verb = "would create"
			}
			return output(cmd, map[string]any{"path": target, "changed": changed, "dryRun": dryRun, "recorded": recorded}, verb+" "+target)
		},
	}
	mutating(touch)
	root.AddCommand(touch)

	root.AddCommand(&cobra.Command{
		Use:     "describe",
		Short:   "Print every command, flag and exit code as JSON",
		Example: "  tool describe",
		Args:    usageArgs(cobra.NoArgs),
		RunE:    func(*cobra.Command, []string) error { return printJSON(describe(root)) },
	})
	return root
}

// capped cuts a list to limit (0: all) and says so on stderr, so a pipe sees
// only results (conventions: JSON output, Human output).
func capped[T any](all []T, limit int) ([]T, map[string]any) {
	items := all
	if limit > 0 && limit < len(all) {
		items = all[:limit]
		fmt.Fprintf(os.Stderr, "%d of %d shown; --limit 0 for all\n", len(items), len(all))
	}
	if items == nil {
		items = []T{}
	}
	return items, map[string]any{"total": len(all), "shown": len(items), "truncated": len(items) < len(all)}
}

// mutating gives a command --dry-run and --yes and marks it, so the root's
// PersistentPreRunE refuses it without exactly one; RunE calls change for
// which.
func mutating(c *cobra.Command) {
	c.Flags().Bool("dry-run", false, "show what would change; change nothing")
	c.Flags().Bool("yes", false, "apply the change")
	if c.Annotations == nil {
		c.Annotations = map[string]string{}
	}
	c.Annotations["mutates"] = "true"
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

// plain refuses an empty argument or a control character in one (rubric A9).
func plain(what, s string) (string, error) {
	if s == "" {
		return "", usageError{fmt.Errorf("%s is empty", what)}
	}
	if strings.IndexFunc(s, unicode.IsControl) >= 0 {
		return "", usageError{fmt.Errorf("%s contains a control character", what)}
	}
	return s, nil
}

// printable makes text from files or the network safe for a terminal.
func printable(s string) string {
	return strings.Map(func(r rune) rune {
		if unicode.IsControl(r) && r != '\t' {
			return '?'
		}
		return r
	}, s)
}

// inside is the file an output path names, refused unless it is plainly
// inside root (rubric A9). Every template's inside() refuses the same
// things: a `..` part, a trailing slash, the root itself, a parent that
// resolves outside root, and a last part that is a symlink (dangling or not),
// so the write can never follow one.
func inside(root, path string) (string, error) {
	refuse := func(format string, a ...any) error {
		return fmt.Errorf("%s: "+format, append([]any{path}, a...)...)
	}
	if slices.Contains(strings.Split(filepath.ToSlash(path), "/"), "..") {
		return "", refuse("'..' is not allowed in an output path")
	}
	if strings.HasSuffix(path, "/") || strings.HasSuffix(path, string(filepath.Separator)) {
		return "", refuse("names a directory, not a file")
	}
	realRoot, err := filepath.EvalSymlinks(root)
	if err != nil {
		return "", err
	}
	full := filepath.Clean(path)
	if !filepath.IsAbs(full) {
		full = filepath.Join(root, full)
	}
	parent, err := filepath.EvalSymlinks(filepath.Dir(full))
	if err != nil {
		return "", refuse("no such directory: %s", filepath.Dir(full))
	}
	target := filepath.Join(parent, filepath.Base(full))
	rel, err := filepath.Rel(realRoot, target)
	switch {
	case err == nil && rel == ".":
		return "", refuse("is %s itself", realRoot)
	case err != nil || rel == ".." || strings.HasPrefix(rel, ".."+string(filepath.Separator)):
		return "", refuse("outside %s", realRoot)
	}
	if fi, err := os.Lstat(target); err == nil && fi.Mode()&os.ModeSymlink != 0 {
		return "", refuse("is a symlink; refusing to write through it")
	}
	return target, nil
}

// home is where config, cache or state live (conventions: State).
// TOOL_HOME moves all three under one directory; tests set it. Empty when
// there is no home to put them in (HOME unset, as under cron): never the
// current directory.
func home(kind string) string {
	if dir := os.Getenv("TOOL_HOME"); dir != "" {
		abs, _ := filepath.Abs(dir)
		return filepath.Join(abs, kind)
	}
	vars := map[string][2]string{
		"config": {"XDG_CONFIG_HOME", ".config"},
		"cache":  {"XDG_CACHE_HOME", ".cache"},
		"state":  {"XDG_STATE_HOME", filepath.Join(".local", "state")},
	}[kind]
	base := os.Getenv(vars[0])
	if !filepath.IsAbs(base) { // XDG says to ignore a relative path
		h, err := os.UserHomeDir()
		if err != nil || !filepath.IsAbs(h) {
			return ""
		}
		base = filepath.Join(h, vars[1])
	}
	return filepath.Join(base, "tool")
}

// homes is the tool's own directories that exist, by real path: a walk
// skips them (conventions: Paths), so a TOOL_HOME inside the tree is not
// listed.
func homes() map[string]bool {
	own := map[string]bool{}
	for _, d := range []string{os.Getenv("TOOL_HOME"), home("config"), home("cache"), home("state")} {
		if d == "" {
			continue
		}
		if real, err := filepath.EvalSymlinks(d); err == nil {
			own[real] = true
		}
	}
	return own
}

// appendState appends one JSON line to a state file (conventions: State).
// Appending never reads the file back, so a corrupt or unreadable one is
// never overwritten, and concurrent runs never lose each other's lines.
// Recording is not the command's action: a failure is a warning, and the
// result says recorded: false.
func appendState(name string, entry any) bool {
	state := home("state")
	if state == "" {
		fmt.Fprintln(os.Stderr, "tool: warning: not recorded: no home directory; set TOOL_HOME or XDG_STATE_HOME")
		return false
	}
	err := os.MkdirAll(state, 0o700)
	var file string
	if err == nil {
		file, err = inside(state, name)
	}
	if err == nil {
		var f *os.File
		if f, err = os.OpenFile(file, os.O_WRONLY|os.O_APPEND|os.O_CREATE, 0o600); err == nil {
			line, _ := json.Marshal(entry)
			_, err = f.Write(append(line, '\n'))
			if cerr := f.Close(); err == nil {
				err = cerr
			}
		}
	}
	if err != nil {
		fmt.Fprintf(os.Stderr, "tool: warning: not recorded in %s: %s\n", filepath.Join(state, name), cause(err))
		return false
	}
	return true
}

// wholeNumber is a flag that takes base-10 digits only, as in every template:
// pflag's own Uint would read 010 as octal and take 0x3 and 1_0.
type wholeNumber struct{ n int }

func (w *wholeNumber) String() string { return strconv.Itoa(w.n) }
func (w *wholeNumber) Type() string   { return "uint" }
func (w *wholeNumber) Set(s string) error {
	if s == "" || strings.Trim(s, "0123456789") != "" {
		return fmt.Errorf("takes a whole number, got %q", s)
	}
	n, err := strconv.Atoi(s)
	if err != nil {
		return fmt.Errorf("%q is too large", s)
	}
	w.n = n
	return nil
}

// cause is an OS error without the path it repeats ("permission denied").
func cause(err error) string {
	if u := errors.Unwrap(err); u != nil {
		return u.Error()
	}
	return err.Error()
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
	flags := []map[string]any{}
	add := func(f *pflag.Flag) {
		if f.Hidden {
			return
		}
		var short, def any
		if f.Shorthand != "" {
			short = f.Shorthand
		}
		kind := "string"
		switch f.Value.Type() {
		case "bool":
			kind = "boolean"
		case "int", "uint", "int64", "uint64":
			kind = "integer"
		}
		if kind != "boolean" && f.DefValue != "" {
			def = f.DefValue
		}
		flags = append(flags, map[string]any{
			"name": f.Name, "short": short, "type": kind, "default": def,
			"description": strings.ReplaceAll(f.Usage, "`", ""), // backquotes name the placeholder
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
	d := map[string]any{"name": c.Name(), "summary": c.Short, "flags": flags, "commands": subs}
	if !c.HasParent() {
		d["version"] = version
		d["exitCodes"] = map[string]int{"ok": exitOK, "failed": exitFailed, "usage": exitUsage, "partial": exitPartial}
		return d
	}
	// Positionals from Use ("files [DIR]"): [X] is optional.
	args := []map[string]any{}
	for _, w := range strings.Fields(c.Use)[1:] {
		name := strings.Trim(w, "[]")
		var def any
		if v, ok := c.Annotations["default."+name]; ok {
			def = v
		}
		args = append(args, map[string]any{"name": name, "required": !strings.HasPrefix(w, "["), "default": def})
	}
	examples := []string{}
	for _, line := range strings.Split(c.Example, "\n") {
		if line = strings.TrimSpace(line); line != "" {
			examples = append(examples, line)
		}
	}
	d["usage"] = c.UseLine()
	d["mutates"] = c.Annotations["mutates"] == "true"
	d["args"] = args
	d["examples"] = examples
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
	// A partial result is already printed, problems included.
	if errors.As(err, new(partialError)) {
		os.Exit(exitPartial)
	}
	fmt.Fprintf(os.Stderr, "tool: %v\n", err)
	usage := errors.As(err, new(usageError))
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

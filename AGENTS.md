# house-skills

Claude Code skills shared across the team, published as a plugin marketplace.

## Layout

- `.claude-plugin/marketplace.json` — the catalog; one entry per plugin.
- `plugins/<plugin>/.claude-plugin/plugin.json` — its `name` must equal the
  directory name and the marketplace entry's `name`.
- `plugins/<plugin>/skills/<skill>/SKILL.md` — skills are auto-discovered;
  never list them in `plugin.json`. Supporting files sit beside it in
  `references/` (read on demand) and `scripts/` (executed).
- `test/<skill>/run` — a skill's tests; `bin/ci` runs them all.

Everything a plugin needs must live inside its directory: installs copy the
plugin, not the repo.

## Writing a skill

- `description` says what the skill does and when to use it, key use case
  first: it is all Claude sees when deciding to load the skill, and the
  listing cuts `description` plus `when_to_use` at 1,536 characters. Leave
  out `/plugin:skill` phrases; people type those, Claude does not need them.
- `disable-model-invocation: true` hides the description from Claude
  entirely and blocks Claude from invoking the skill. Use it for skills that
  are long-running, costly or act on shared state — and never tell another
  skill to invoke one; tell it to ask the user instead.
- Keep `SKILL.md` to the procedure, under 500 lines. Rationale and long rules
  go in `references/`, linked from where they apply.
- Write standing instructions, not one-time steps: an invoked skill stays in
  context and is never re-read. Put what matters most near the top; after
  compaction only the first 5,000 tokens are re-attached.
- A rule that must hold every time belongs in a hook (the skill's `hooks`
  frontmatter or the plugin's `hooks/`), not in prose.
- A full-auto skill sets `disallowed-tools: AskUserQuestion`. `allowed-tools`
  grants last only until the user's next message.
- Put in a script whatever must be exact and the same every time — API reads,
  trust decisions, waiting. Leave judgment to the model. A script prints JSON
  and gives distinct exit codes to "nothing found" and "could not look", so an
  empty answer is never a failure in disguise.
- A script that reads text other people wrote decides whose text the agent may
  see; that decision gets tests.
- Scripts run under macOS's bash 3.2: no associative arrays, no `mapfile`.
- Configuration that grants trust comes from the environment, never from a file
  in the repository the skill is working on.

## Releasing

Plugins omit `version`, so every commit on `main` is a release. Never rename a
plugin's `name`; add a `renames` entry in `marketplace.json` if you must.

## Before pushing

    bin/ci

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

- `description` says what the skill does and when to use it — it is all
  Claude sees when deciding to load the skill. Skills that are long-running,
  costly or act on shared state set `disable-model-invocation: true`.
- Keep `SKILL.md` to the procedure. Rationale and long rules go in
  `references/`, linked from where they apply.
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

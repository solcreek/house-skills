# The describe shape

Every template's `describe` prints one JSON document in this shape, so an
agent that has read one of our tools can read any of them. Keep the shape
when you add commands; add fields, never rename them.

```json
{
  "name": "tool",
  "version": "0.1.0",
  "summary": "one line on what it is for",
  "exitCodes": { "ok": 0, "failed": 1, "usage": 2 },
  "flags": [{ "name": "help", "short": "h", "type": "boolean", "default": null, "description": "…" }],
  "commands": [
    {
      "name": "files",
      "summary": "list the files under a directory, largest first",
      "usage": "tool files [DIR] [options]",
      "mutates": false,
      "args": [{ "name": "DIR", "required": false, "default": "." }],
      "flags": [
        { "name": "limit", "short": null, "type": "integer", "default": "20", "description": "…" },
        { "name": "json", "short": null, "type": "boolean", "default": null, "description": "…" }
      ],
      "examples": ["tool files", "tool files src --limit 5 --json"],
      "commands": []
    }
  ]
}
```

- The top level has exactly `name`, `version`, `summary`, `exitCodes`,
  `flags`, `commands`; every command has exactly `name`, `summary`, `usage`,
  `mutates`, `args`, `flags`, `examples`, `commands`. Every key is always
  present; an absent value is `null` or `[]`.
- A command's `flags` include the ones it inherits (`--json`, `--help`): what
  its `--help` shows.
- `mutates` is on every command: true means it takes `--dry-run` and `--yes`
  and refuses to run without exactly one (conventions).
- A flag's `type` is `boolean`, `integer` (its placeholder in help is `N`)
  or `string`. `default` is the default of a flag that takes a value, else
  null, always as a string.
- `args` are the positionals in order; an optional one may have a
  `default`.
- `test/new/run` checks this shape for every template.

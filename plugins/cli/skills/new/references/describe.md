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
  "flags": [{ "name": "help", "short": "h", "takesValue": false, "default": null, "description": "…" }],
  "commands": [
    {
      "name": "greet",
      "summary": "print a greeting",
      "usage": "tool greet NAME [options]",
      "mutates": false,
      "flags": [{ "name": "json", "short": null, "takesValue": false, "default": null, "description": "…" }],
      "commands": []
    }
  ]
}
```

- `version` and `exitCodes` appear at the top level only.
- A command's `flags` include the ones it inherits (`--json`, `--help`): what
  its `--help` shows.
- `mutates` is on every command: true means it takes `--dry-run` and `--yes`
  and refuses to run without exactly one (conventions).
- `default` is the default of a flag that takes a value, else null.
- Optional per command: `examples` (the TypeScript template has them).
- `test/new/run` checks this shape for every template.

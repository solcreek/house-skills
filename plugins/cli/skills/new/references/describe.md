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
  "flags": [{ "name": "help", "short": "h", "takesValue": false, "description": "…" }],
  "commands": [
    {
      "name": "greet",
      "summary": "print a greeting",
      "usage": "tool greet NAME [--json]",
      "flags": [{ "name": "json", "short": null, "takesValue": false, "description": "…" }],
      "commands": []
    }
  ]
}
```

- `version` and `exitCodes` appear at the top level only.
- A command's `flags` include the ones it inherits (`--json`, `--help`): what
  its `--help` shows.
- Optional per command: `mutates` (true: takes `--yes` and `--dry-run`),
  `examples`. The TypeScript template has both; in Rust and Go, add them when
  a command mutates.
- `test/new/run` checks this shape for every template.

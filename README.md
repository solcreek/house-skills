# house-skills

Shared Claude Code skills for the team, distributed as a
[plugin marketplace](https://code.claude.com/docs/en/plugins/create-marketplace).

| Plugin | Skill | What it does |
|---|---|---|
| `dev` | `/dev:babysit-pr [PR]` | Drives a PR to mergeable: fixes CI, merges in the base on conflicts, addresses trusted review feedback, replies and resolves threads, and waits for checks and Copilot in the background |

## Install

The repository is private, so git on your machine must reach GitHub without a
prompt: an SSH key in `ssh-agent`, or

```bash
gh auth login && gh auth setup-git
```

Then, in Claude Code:

```
/plugin marketplace add solcreek/house-skills
/plugin install dev@house-skills
```

Turn on updates once: `/plugin` → **Marketplaces** → `house-skills` →
**Enable auto-update**. Or update by hand with
`/plugin marketplace update house-skills`.

## Configure babysit-pr

`babysit-pr` acts only on feedback from people it trusts. Set the orgs whose
owners and members it trusts in `~/.claude/settings.json` on each machine:

```json
{
  "env": {
    "BABYSIT_ORGS": "solcreek,kaikhq,loopwise,ovid-sh,momiji-rs"
  }
}
```

- People the repo calls OWNER or MEMBER in those orgs, and your own account,
  are trusted. Everyone else's comments are listed as links for you, never read.
- Copilot, Codex, GitHub Advanced Security and Claude are trusted review bots.
  Add more with `BABYSIT_BOTS` (comma-separated GitHub App logins); other bots'
  comments are ignored.
- Without `BABYSIT_ORGS`, only your own comments and the review bots count.
- PRs authored outside those orgs, from forks, or labeled `outside-text` are
  refused.

Requirements: `gh` (authenticated), `jq`, `git`.

## Contribute

See [AGENTS.md](AGENTS.md). Run `bin/ci` before pushing.

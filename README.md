# house-skills

Shared Claude Code skills for the team, distributed as a
[plugin marketplace](https://code.claude.com/docs/en/plugins/create-marketplace).

| Plugin | Skill | What it does |
|---|---|---|
| `dev` | `/dev:babysit-pr [PR]` | Drives a PR to mergeable: fixes CI, merges in the base on conflicts, addresses trusted review feedback, replies and resolves threads, and waits for checks and Copilot in the background |
| `recap` | `/recap:session [focus]` | Where this session stands and what to do next: done (with evidence checked against git and the PR), in progress, not verified, needs you, and ranked next steps. Loads on its own when you ask for a progress summary |
| `ystack` | playbooks, verification skills, evidence gate | Agent-native dev workflow, maintained in [solcreek/ystack](https://github.com/solcreek/ystack) |

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
/plugin install recap@house-skills
/plugin install ystack@house-skills
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

**Copilot review.** Where a ruleset on the base branch already reviews new PRs
with Copilot, the skill waits for that review. Where none does, it asks Copilot
for one review itself. Either way it asks again (up to 3 times a run) after it
pushes fixes. Copilot reviews are billed per review, so set
`"BABYSIT_COPILOT": "never"` to stop the skill from asking at all; rulesets
still trigger their own reviews.

**Your own reviewer.** Before each push the skill reviews its own new
commits, with a subagent by default. A repository that has its own review
command declares it in `REVIEW.md`, `AGENTS.md` or `CLAUDE.md` (the first of
those with one):

````markdown
```babysit-pr-reviewer
pnpm -s review:ai --base {since} --format text
```
````

One command line; `{since}` becomes the first commit the review must not
cover. It exits 0 when the review ran to completion, whatever it found. The
declaration is read from the base branch, so a PR cannot change which command
runs. The command itself runs in the PR's checkout, on the PR's code, like the
build and tests the skill already runs: a PR that changes the scripts behind
it changes the review too.

**Code scanning.** The skill reads the PR's open code-scanning alerts, not
only the ones the code-scanning bot comments on, and treats them as feedback
once the analysis has finished. It never dismisses an alert.

**After `gh pr create`.** When Claude opens a PR with `gh pr create`, a hook
in the `dev` plugin has it offer `/dev:babysit-pr <url>`. It only suggests;
the skill is manual-only and starts when you type it. Set
`"BABYSIT_SUGGEST": "never"` to turn the suggestion off.

Requirements: `gh` (authenticated), `jq`, `git`.

**Permissions.** The skill pre-approves its own scripts, `git` and `gh`, but
Claude Code clears a skill's `allowed-tools` grant when you send your next
message, and the loop also runs your repo's build and test commands. For an
unattended run, use auto or bypass permission mode, or allow `Bash(git *)`,
`Bash(gh *)` and your test commands in `~/.claude/settings.json`.

## Contribute

See [AGENTS.md](AGENTS.md). Run `bin/ci` before pushing.

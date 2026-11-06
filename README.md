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

**Your repository's own commands.** A repository that has its own tool for a
job the skill does declares it, and the skill runs that command instead of
working one out from the docs. Each declaration is a fenced block named
`babysit-pr-<capability>` holding one command line, in `REVIEW.md`,
`AGENTS.md` or `CLAUDE.md` (for each capability, the first of those that
declares it):

````markdown
```babysit-pr-checks
bin/mk pr status {number} --wait --json
```
````

| Capability | The command | Placeholders | Undeclared, the skill uses |
|---|---|---|---|
| `checkout` | gets onto the PR's head branch | `{number}`, `{branch}` | `gh pr checkout` |
| `checks` | prints the PR's checks, waiting while any runs | `{number}` | `pr-state` and `gh` |
| `ci-failures` | prints the failed checks' error lines | `{number}` | `gh run view --log-failed`, the last lines of each failed job |
| `gate` | runs the pre-push gates (lint, types, ...) on HEAD | `{base}` | nothing |
| `test` | runs the tests the change since `{since}` affects | `{since}`, `{base}` | the narrowest tests it can find in the docs |
| `reviewer` | reviews the commits after `{since}`; exits 0 when the review ran to completion, whatever it found | `{since}` (required), `{base}` | a review subagent |
| `pr-open` | opens a PR (see "After `gh pr create`") | none | `gh pr create` only |

The skill fills in each placeholder, shell-quoted when the value is not plain;
leave them unquoted. A placeholder the capability does not take, two blocks
for one capability in a file, or a block that is not one line breaks that
declaration: the skill falls back for it and says why in its report. Run
`plugins/dev/skills/babysit-pr/scripts/repo-commands <base>` from a clone to
see what it reads.

Declarations are read from the base branch, so a PR cannot change which
command runs. The command itself runs in the PR's checkout, on the PR's code,
like the build and tests the skill already runs: a PR that changes the scripts
behind it changes what the command does. The trust filter (`pr-state`) and
the wait between cycles (`wait-pr`) are never replaced.

**A branch held by another worktree.** When the PR's branch is checked out in
another `git worktree`, the skill puts the PR's head on a local branch named
`babysit-pr-<number>` instead. It always pushes with
`git push origin HEAD:<branch>`, so the local name never matters.

**Code scanning.** The skill reads the PR's open code-scanning alerts, not
only the ones the code-scanning bot comments on, and treats them as feedback
once the analysis has finished. It never dismisses an alert.

**After `gh pr create`.** When Claude opens a PR with `gh pr create`, a hook
in the `dev` plugin has it offer `/dev:babysit-pr <url>`. It only suggests;
the skill is manual-only and starts when you type it. Set
`"BABYSIT_SUGGEST": "never"` to turn the suggestion off. A repository that
opens PRs through its own wrapper declares it as the `pr-open` capability
above, for example `bin/mk pr open`. The words before the first option or
placeholder name the opener, its first one matched by name, so `mk pr open`,
`bin/mk pr open` and `./bin/mk pr open --fill` all count. The hook reads the
declaration from the checkout the command ran in; the suggestion still needs a
PR URL in the command's output.

Requirements: `gh` (authenticated), `jq`, `git`.

**Permissions.** The skill pre-approves its own scripts, `git` and `gh`, but
Claude Code clears a skill's `allowed-tools` grant when you send your next
message, and the loop also runs your repo's build and test commands. For an
unattended run, use auto or bypass permission mode, or allow `Bash(git *)`,
`Bash(gh *)` and your test commands in `~/.claude/settings.json`.

## Contribute

See [AGENTS.md](AGENTS.md). Run `bin/ci` before pushing.

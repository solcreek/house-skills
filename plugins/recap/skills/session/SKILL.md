---
name: session
description: "Recap the current session: what it set out to do, what is done (with evidence checked against git and the PR right now), what is uncommitted or unpushed, what was claimed but never verified, what needs the user's decision, and a ranked list of next steps. Use when the user asks where things stand, for a progress summary, a status update, or what to do next (e.g. 「彙整一下目前的進度」「接下來可以推進什麼」), or before handing work off or clearing context."
argument-hint: "[focus]"
allowed-tools:
  - Bash(${CLAUDE_SKILL_DIR}/scripts/*)
---

# Session recap

Answer "where are we, and what next" for this session. Ground every claim in
what is true now, not in what the conversation remembers: after a long
session or a compaction, memory of "done" is the least reliable thing in
context.

If the user gave a focus, `$ARGUMENTS`, recap only that thread of work.

## 1. Read the actual state

Run `${CLAUDE_SKILL_DIR}/scripts/state DIR...` with every git checkout this
session read or changed (worktrees included); with none named, it reads the
current directory. It prints JSON per repo: branch, uncommitted files,
`unpushed` commits (null when the branch has no upstream, so nothing is
pushed), `commitsOnBranch` since the base, worktrees, and the branch's PR with
its checks (`pr` is null when there is none; `{"unknown": ...}` when gh could
not answer — say so, never read it as "no PR").

Exit codes: **0** state · **3** no directory was a git repository: the
session's work is not in git, so recap from the conversation and say nothing
could be checked · **1** the read failed: report it and recap from the
conversation, marking every item unchecked.

Also account for what the script cannot see: background tasks and agents
still running, scheduled wakeups, servers you started, files written outside
a repo, and external actions (deploys, messages, issues filed).

## 2. Reconcile

Match each thing the session did or said it did against the state:

- **Done** needs evidence a reader can check: a commit SHA, a PR number, a
  passing check, a test run's output seen in this session. Work that exists
  only as uncommitted files is in progress, not done.
- **Claimed, not verified**: anything stated as working, fixed or tested
  without a run that showed it. Include tests you skipped, checks still
  pending, and fixes pushed whose CI has not finished.
- **Drift**: state that contradicts the conversation — a commit you remember
  that is not there, a PR head that is not your local head
  (`pushedHeadIsLocalHead: false`), a failing check, files changed that the
  session never mentioned (another session may share the repo; say so, do not
  touch them).

## 3. Write the recap

Write in the language of the conversation. Keep it to one screen; drop a
section that would be empty, except **Next**.

1. **Goal** — one sentence: what this session is for. If the goal changed
   along the way, say what it is now.
2. **Done** — each with its evidence, inline (`abc1234`, #12, "bin/ci passed").
3. **In progress** — uncommitted, unpushed, or waiting (CI, review, a
   background task), with what it is waiting on.
4. **Not verified** — claims from step 2, each with how to check it.
5. **Needs you** — decisions only the user can make, blockers outside your
   reach, anything that needs their credentials or approval.
6. **Next** — ranked, at most five. Lead with one clear recommendation and
   why it comes first. Mark each item **I can do this now** or **needs your
   OK** (anything outward-facing or hard to undo: pushing, merging, deploying,
   messaging, deleting). Include finishing what is unverified before starting
   anything new.

End by offering to start the first item. Do not start it unprompted.

Do not run anything that changes state while recapping: no commits, pushes,
fetches or fixes. A recap is read-only.

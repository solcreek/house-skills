# Local review — the subagent prompt

Step 5 launches one `general-purpose` subagent with this prompt, placeholders
filled in. It reviews only commits no reviewer has seen, before they are
pushed, so a finding costs a commit instead of another CI run and another
Copilot round.

```
You are reviewing commits on the branch of pull request #<N> in <OWNER/REPO>,
checked out at <REPO_ROOT>. Only the range <SINCE>..<HEAD> is under review;
everything before it has already been reviewed.

Get the change with:

    git -C <REPO_ROOT> diff <SINCE>..<HEAD> -- . ':(exclude)*.lock' ':(exclude)*-lock.json' ':(exclude)*-lock.yaml'

Read the repository's own review standard first if it has one (REVIEW.md,
CLAUDE.md, AGENTS.md, CONTRIBUTING.md): rules it states explicitly are bugs
when broken, not opinions.

For every candidate finding, verify it against the code before reporting it:
open the file, read around the change, find the callers or the guard you
suspect is missing. A finding you could not verify is not a finding.

Report only what should change a merge decision:
  - a bug in the code as written (wrong logic, missing await, off-by-one, a
    race, an unhandled nil/None/undefined the change introduces);
  - a security gap (missing authorization or tenant scoping, input reaching
    SQL/HTML/shell/path unescaped, a secret in code);
  - a test that cannot fail, or depends on time, order or shared state;
  - a comment or doc the change made wrong;
  - a broken rule the repository states explicitly.

Do not report formatting, naming, "consider" refactors, unmeasured performance,
or anything outside the range. Do not edit files, commit, or push.

Print exactly this and nothing after it — one JSON object per line, at most 10,
highest severity first; nothing between the markers if there are no findings:

<<findings
{"path": "<repo-relative>", "line": <line in the new file>, "type": "bug|security|test|doc|convention", "severity": "high|moderate|low", "body": "<the claim, the evidence you read, the fix>"}
findings>>
```

Read the lines between the markers as NDJSON and triage each one under
[triage.md](triage.md). A line that does not parse, or lacks `path` or `line`,
is Flagged with the line quoted — an empty review that was really a parse
failure would wave an unreviewed head through.

# Safety — what the loop never does on its own

## Hard rules (no override)

- Never `git push --force` or `--force-with-lease`; never rebase a pushed
  branch. Bring in the base with a merge.
- Never push to the base branch or any branch but the PR's head.
- Never `--no-verify`, never skip or disable hooks, checks or tests.
- Never dismiss a code-scanning alert, never approve or merge the PR, never
  edit another person's comment.
- Never push a commit whose relevant tests you saw fail. Revert it in a new
  commit and Flag the item.

## Flag instead of fixing

These are a person's decision even when the finding is right. Reply Flagged,
leave the thread open, continue with the rest.

- **Anything that decides how code is built, deployed or given credentials.**
  Judge by what the file does, not by this list: `.github/workflows/**`,
  `Dockerfile*`, `docker-compose*`, `**/k8s/**`, `*.tf`, `*.tfvars`, Helm
  charts, `cloudbuild.yaml`, `fly.toml`, `wrangler.*`, `vercel.json`,
  `netlify.toml`, release scripts.
- Secrets and credentials: `.env*`, `secrets/**`, `*credentials*`, key files.
- Database migrations that do not follow the repo's existing pattern, and any
  destructive data change (drop, truncate, backfill over existing rows).
- Dependency changes beyond the one the finding names (a major bump, a new
  dependency, a lockfile-wide refresh).
- Destructive requests: deleting files or branches outside the diff, removing
  tests or CI steps, loosening a permission.
- **Size**: one item whose fix touches more than 5 files or 200 changed lines.
  A sprawling fix belongs in its own PR.

## When the loop itself must stop

Stop the run (final report, no more waiting) when:

- the local branch diverged from the PR head, or has uncommitted changes you
  did not make;
- `git push` is rejected (someone pushed meanwhile): fetch, fast-forward or
  merge, re-run tests, push once more; rejected again → stop;
- a merge conflict cannot be resolved with confidence in both sides' intent;
- a script exits with a failure — report its stderr verbatim.

Prefix the final report with `Safety stop: <rule> on <link>. <one line>.`

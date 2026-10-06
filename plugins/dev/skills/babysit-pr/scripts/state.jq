# Turns the raw GraphQL pages pr-state fetched into the one JSON object the
# skill reads. Everything that decides whose text reaches the agent lives here.
#
# Inputs (--slurpfile): $head, $reviews, $comments, $threads, $timeline, $checks, $runs,
#                       $rules (the base branch's rules, or null when unreadable)
# Inputs (--arg):       $orgs, $bots — comma-separated, from the environment only
# Inputs (--argjson):   $now — epoch seconds
#
# Trust: a person is inside when the repo belongs to one of $orgs and GitHub
# calls them OWNER or MEMBER there; on any other repo only the token's own
# account is inside. A bot is inside when it is a GitHub App (typename Bot,
# which a user account cannot be) whose login is on the bot list. Outside
# people's text never appears in the output, only a link to it. Outside bots
# are counted and otherwise ignored: they are CI and deploy noise, and a bot
# can repeat outside text, so nothing they wrote is read.

def csv: ascii_downcase | split(",") | map(gsub("^\\s+|\\s+$"; "")) | map(select(length > 0));
def default_bots: ["copilot-pull-request-reviewer", "copilot", "chatgpt-codex-connector",
                   "github-advanced-security", "claude"];
def org_list: $orgs | csv;
def bot_list: default_bots + ($bots | csv);
def copilot: "copilot-pull-request-reviewer";

def h: $head[0].data;
def repo: h.repository;
def pr: repo.pullRequest;
def viewer: h.viewer.login;
def in_company: (repo.owner.login | ascii_downcase) as $o | org_list | index($o) != null;

def login: .author.login // null;
def is_bot: .author.__typename == "Bot";
def is_viewer: login == viewer;
def inside_person: .author.__typename == "User"
  and (is_viewer or (in_company and ((.authorAssociation // "") | IN("OWNER", "MEMBER"))));
def inside_bot: is_bot and ((login // "" | ascii_downcase) as $l | bot_list | index($l) != null);
def inside: inside_person or inside_bot;
def outside_person: (is_bot | not) and (inside_person | not);
# What the agent itself posted: the token's account plus the tag every reply
# and summary carries. The same account's untagged text is the person running
# the loop, and that is work like anyone else's.
def by_agent: is_viewer and ((.body // "") | contains("<!-- babysit-pr"));

def nodes($pages; f): [$pages[0][] | f | .nodes[]?];

# Unicode tag characters (U+E0000-U+E007F) render as nothing on GitHub, so text
# carrying them says something its author never saw. Strip that block only.
def clean: gsub("[\\x{E0000}-\\x{E007F}]+"; "");
def item: {id, url, author: login, body: (.body | clean)};
def link($kind): {kind: $kind, url, author: login};

# Markers count only in the agent's own comments, so nobody else can mark an
# item handled or move the reviewed sha.
def mine: nodes($comments; .data.repository.pullRequest.comments) | map(select(by_agent));
def handled: [mine[].body | scan("<!-- babysit-pr:handled ([^>]*)-->") | .[0] | splits("\\s+") | select(length > 0)];
def reviewed_sha: [mine | sort_by(.createdAt)[].body | scan("<!-- babysit-pr:reviewed ([0-9a-f]{7,40}) -->") | .[0]] | last;

def refusal:
  if pr == null then "the pull request was not found or the token cannot read it"
  elif in_company and (repo.owner.viewerIsAMember | not) then
    "the token is not a member of \(repo.owner.login), so every member would read as an outsider"
  elif pr.isCrossRepository then "the head branch lives in a fork; pushing to forks is not supported"
  elif ([pr.labels.nodes[].name] | index("outside-text")) then
    "the PR is labeled outside-text; a person removes the label after reading the diff"
  elif (pr | inside | not) then "the PR author \(pr.author.login // "ghost") is outside the trusted orgs, so the diff itself is outside text"
  else null end;

# One entry per check, newest attempt wins: the rollup keeps superseded and
# re-run attempts on the same commit, and an old failure must not outlive its
# re-run. A workflow triggered by both push and pull_request runs the same
# check twice on one commit; those are separate checks, keyed by event, so a
# pass under one never hides a failure under the other.
def checks:
  nodes($checks; .data.repository.object.statusCheckRollup.contexts)
  | map(if .__typename == "CheckRun" then
          {name: ((.checkSuite.workflowRun.workflow.name // .checkSuite.app.slug // "") + " / " + .name),
           at: (.startedAt // ""),
           pending: (.status != "COMPLETED"),
           failed: (.status == "COMPLETED" and ((.conclusion // "") | IN("FAILURE", "TIMED_OUT", "CANCELLED", "ACTION_REQUIRED", "STARTUP_FAILURE", "STALE"))),
           conclusion: (.conclusion // .status),
           event: .checkSuite.workflowRun.event,
           run_id: .checkSuite.workflowRun.databaseId,
           url: .detailsUrl}
        else
          {name: .context, at: (.createdAt // ""),
           pending: (.state | IN("PENDING", "EXPECTED")),
           failed: (.state | IN("ERROR", "FAILURE")),
           conclusion: .state, event: null, run_id: null, url: .targetUrl}
        end)
  | group_by([.name, .event]) | map(sort_by(.at) | last)
  | {failing: map(select(.failed) | del(.at, .pending, .failed) | if .event == null then del(.event) else . end),
     pending: map(select(.pending) | .name),
     passed: map(select((.pending or .failed) | not)) | length};

# Copilot is working when its newest review request is newer than both its
# newest review and any removal of the request. reviewRequests cannot say this:
# it empties the moment Copilot picks a request up, long before it posts.
#
# The review itself runs as an Actions run (path below). A run that finished
# after the request without a review being posted means no review is coming:
# failed or cancelled at once, succeeded after a grace period, since the
# review can land a little after the run completes.
def copilot_run_path: "dynamic/agents/copilot-pull-request-reviewer";
def review_grace: 600;
# A copilot_code_review rule on the base branch means GitHub requests Copilot
# on every new PR by itself; null when the rules could not be read.
def copilot_auto: if $rules[0] == null then null
  else any($rules[0][]?; .type == "copilot_code_review") end;
def copilot_state:
  (nodes($reviews; .data.repository.pullRequest.reviews) | map(select(login == copilot)) | sort_by(.createdAt)) as $rv
  | [nodes($timeline; .data.repository.pullRequest.timelineItems)[]
     | select(.requestedReviewer.login? == copilot)] as $ev
  | ($ev | map(select(.__typename == "ReviewRequestedEvent") | .createdAt) | max) as $asked
  | ([($rv | last | .createdAt), ($ev | map(select(.__typename == "ReviewRequestRemovedEvent") | .createdAt) | max)]
     | map(select(. != null)) | max) as $answered
  | ($asked != null and ($answered == null or $asked > $answered)) as $open
  | ([$runs[0][] | select(.path == copilot_run_path and $asked != null and .created_at >= $asked)]
     | sort_by(.created_at) | last) as $run
  | (if $open and $run != null and $run.status == "completed" then
       if $run.conclusion != "success" then "the Copilot review run ended \($run.conclusion) without posting a review"
       elif ($now - ($run.updated_at | fromdateiso8601)) > review_grace then "the Copilot review run succeeded \(review_grace / 60) minutes ago but posted no review"
       else null end
     else null end) as $gave_up
  | {involved: (($ev | length) > 0 or ($rv | length) > 0),
     reviews: ($rv | length),
     pending: (if $open and $gave_up == null then 1 else 0 end),
     no_review: $gave_up,
     run_url: $run.html_url?,
     reviewed_sha: ($rv | last | .commit.oid? // null)}
  | . + {stale: (.involved and .reviewed_sha != pr.headRefOid), auto: copilot_auto}
  # The repo does not ask Copilot by itself, so the agent asks once.
  | . + {first_request: ((.involved | not) and .auto == false)};

def threads:
  nodes($threads; .data.repository.pullRequest.reviewThreads) | map(select(.isResolved | not))
  | map(. as $t | {id, path, line, outdated: .isOutdated,
          url: (.comments.nodes[0].url // null),
          complete: (.comments.totalCount == (.comments.nodes | length)),
          comments: .comments.nodes})
  | {withheld: map(select((.complete | not) or any(.comments[]; inside | not)) | {kind: "thread", url, author: (.comments | map(select(inside | not)) | first | login)}),
     ok: map(select(.complete and all(.comments[]; inside)))}
  | {withheld,
     # The agent spoke last: it answered and left the thread open for a
     # person, or said "Fixed" and the thread waits for green CI.
     awaiting: (.ok | map(select(.comments | last | by_agent)
                 | {id, path, url, reply: (.comments | last | .body | clean | .[0:120])})),
     todo: (.ok | map(select(.comments | last | by_agent | not)
                 | {id, path, line, outdated, url, comments: (.comments | map({author: login, body: (.body | clean)}))}))};

def bodies($kind; f):
  handled as $done
  | f | map(select(by_agent | not) | select((.body // "" | clean | gsub("\\s"; "")) != ""))
  | map(select(.id as $id | $done | index($id) | not))
  | {todo: map(select(inside)), withheld: map(select(outside_person) | link($kind)), ignored: map(select(is_bot and (inside_bot | not))) | length};

if refusal then {refused: refusal, url: pr.url?}
else
  threads as $th
  | bodies("review"; nodes($reviews; .data.repository.pullRequest.reviews) | map(select(.state | IN("COMMENTED", "CHANGES_REQUESTED")))) as $rv
  | bodies("comment"; nodes($comments; .data.repository.pullRequest.comments)) as $cm
  | checks as $ck
  | copilot_state as $cp
  | {pr: pr.number, url: pr.url, state: pr.state, draft: pr.isDraft,
     head: pr.headRefOid, branch: pr.headRefName, base: pr.baseRefName,
     mergeable: pr.mergeable, merge_state: pr.mergeStateStatus,
     viewer: viewer, reviewed_sha: reviewed_sha,
     checks: $ck,
     copilot: $cp,
     requested: [pr.reviewRequests.nodes[].requestedReviewer | .login // .slug],
     threads: $th.todo,
     reviews: ($rv.todo | map(item + {state, commit: .commit.oid})),
     comments: ($cm.todo | map(item)),
     awaiting: $th.awaiting,
     withheld: ($th.withheld + $rv.withheld + $cm.withheld),
     ignored_bot_items: ($rv.ignored + $cm.ignored)}
  | .settled = ((.checks.pending | length) == 0 and .copilot.pending == 0 and .mergeable != "UNKNOWN")
  | .work = ((.threads | length) + (.reviews | length) + (.comments | length) + (.checks.failing | length)
             + (if .mergeable == "CONFLICTING" or .merge_state == "BEHIND" then 1 else 0 end))
  | .done = (.state == "OPEN" and .settled and .work == 0 and .mergeable == "MERGEABLE")
end

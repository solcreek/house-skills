# Sourced by probe and run-json (bash 3.2). Expects $tmp, $cwd, $timeout,
# $results and $offline to be set by the caller, and the array runenv
# (KEY=VALUE pairs from --env; may be empty).

# With --offline, every run gets proxies that point nowhere and git that
# refuses every transport. Best effort, not a sandbox: a tool that ignores
# proxy variables still reaches the network.
OFFLINE_ENV="HTTP_PROXY=http://127.0.0.1:9 HTTPS_PROXY=http://127.0.0.1:9 ALL_PROXY=http://127.0.0.1:9
http_proxy=http://127.0.0.1:9 https_proxy=http://127.0.0.1:9 all_proxy=http://127.0.0.1:9
NO_PROXY= no_proxy= GIT_ALLOW_PROTOCOL=none"

# run NAME ARGV... : runs ARGV in $cwd under the timeout, stdin at EOF, with
# no colour forcing or NO_COLOR from the caller. Kills the whole process
# group on timeout. Leaves $tmp/NAME.{out,err,meta}; meta is "EXIT MS TIMEDOUT".
run() {
  local name=$1; shift
  local extra=""
  [ -n "${offline:-}" ] && extra=$OFFLINE_ENV
  # shellcheck disable=SC2086
  # ${a[@]+"${a[@]}"}: an empty array under set -u in bash 3.2.
  (cd "$cwd" && env -u FORCE_COLOR -u CLICOLOR_FORCE -u NO_COLOR $extra ${runenv[@]+"${runenv[@]}"} \
    perl -MTime::HiRes=time -e '
      my ($secs, $meta) = (shift, shift);
      my $t0 = time;
      my $pid = fork // die "fork: $!";
      if ($pid == 0) { setpgrp(0, 0); exec { $ARGV[0] } @ARGV; exit 127 }
      my $timed = 0;
      $SIG{ALRM} = sub { $timed = 1; kill "KILL", -$pid };
      alarm $secs;
      while (waitpid($pid, 0) == -1 && $!{EINTR}) {}
      my $st = $?;
      alarm 0;
      kill "KILL", -$pid if $timed;
      my $code = ($st & 127) ? 128 + ($st & 127) : $st >> 8;
      open my $fh, ">", $meta or die; printf $fh "%d %d %d\n", $code, (time - $t0) * 1000, $timed;
    ' "$timeout" "$tmp/$name.meta" "$@" < /dev/null > "$tmp/$name.out" 2> "$tmp/$name.err") || true
}

# bytes FILE: size without the padding BSD wc adds.
bytes() { wc -c < "$1" | tr -d ' '; }

# argv ARGS...: a JSON array of ARGS. Each goes through --arg, never as a jq
# option: an argument like --help must stay data.
argv() { local a; for a in "$@"; do jq -n --arg a "$a" '$a'; done | jq -cs .; }

# meta NAME: sets exit, ms, timed from the run.
meta() { read -r exit ms timed < "$tmp/$1.meta"; }

# has_ansi NAME: the run wrote an escape character to stdout or stderr.
has_ansi() { grep -q "$(printf '\033')" "$tmp/$1.out" "$tmp/$1.err"; }

# runjson NAME ARGV... : the run as JSON, output cut to 2000 bytes.
runjson() {
  local name=$1; shift
  local exit ms timed
  meta "$name"
  jq -n --argjson exit "$exit" --argjson ms "$ms" --argjson timed "$timed" \
    --argjson ob "$(bytes "$tmp/$name.out")" --argjson eb "$(bytes "$tmp/$name.err")" \
    --arg o "$(head -c 2000 "$tmp/$name.out")" --arg e "$(head -c 2000 "$tmp/$name.err")" \
    --argjson argv "$(argv "$@")" \
    '{argv: $argv, exit: $exit, ms: $ms, timedOut: ($timed == 1),
      stdoutBytes: $ob, stderrBytes: $eb, stdout: $o, stderr: $e}'
}

# record ID STATUS DETAIL [RUNJSON]
record() {
  jq -cn --arg id "$1" --arg s "$2" --arg d "$3" --argjson r "${4:-null}" \
    '{id: $id, status: $s, detail: $d} + (if $r then {run: $r} else {} end)' >> "$results"
}

# status ID: the recorded status of check ID, or nothing.
status() { jq -r --arg id "$1" 'select(.id == $id) | .status' "$results"; }

# demote ID DETAIL: turn a recorded pass into a fail with DETAIL.
demote() {
  jq -c --arg id "$1" --arg d "$2" \
    'if .id == $id then .status = "fail" | .detail = $d else . end' "$results" > "$tmp/demote" &&
    mv "$tmp/demote" "$results"
}

# envarg KEY=VALUE: append to runenv, or exit 64 if it is not one.
envarg() {
  case "$1" in [A-Za-z_]*=*) runenv+=("$1") ;; *) usage ;; esac
}

# resolve CMD: an absolute path for a path, as given for a name on PATH.
# Prints JSON {error} and exits 2 when it is neither.
resolve() {
  case "$1" in
    */*) if [ -f "$1" ] && [ -x "$1" ]; then echo "$(cd "$(dirname "$1")" && pwd)/$(basename "$1")"
         else jq -n --arg e "not an executable file: $1" '{error: $e}'; exit 2; fi ;;
    *) if command -v "$1" > /dev/null; then echo "$1"
       else jq -n --arg e "command not found: $1" '{error: $e}'; exit 2; fi ;;
  esac
}

# report BASE...: prints the result JSON; exits 1 if any check failed.
# With $brief set, each run shrinks to its exit, time and first stderr line.
report() {
  case "$cwd" in "$tmp"/*) cwd="(a temporary empty directory)" ;; esac
  jq -s --arg cwd "$cwd" --arg brief "${brief:-}" --argjson offline "$([ -n "${offline:-}" ] && echo true || echo false)" '
  (if $brief == "" then . else map(if .run then .run |= {exit, ms, timedOut,
      stderr: (.stderr | split("\n") | map(select(length > 0)) | .[0] // "")} else . end) end) |
  {
    cwd: $cwd,
    offline: $offline,
    summary: (group_by(.status) | map({key: .[0].status, value: length}) | from_entries),
    checks: .
  }' "$results" > "$tmp/body"
  jq -n --slurpfile b "$tmp/body" --argjson c "$(argv "$@")" '{command: $c} + $b[0]'
  if jq -se 'any(.status == "fail")' "$results" > /dev/null; then exit 1; fi
  exit 0
}

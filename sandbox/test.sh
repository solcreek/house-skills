#!/usr/bin/env bash
set -euo pipefail
here=$(cd "$(dirname "$0")" && pwd)
[ "$("$here/sum.sh" 1 2)" = "3" ] || { echo "expected 1+2=3, got $("$here/sum.sh" 1 2)"; exit 1; }
[ "$("$here/sum.sh" 5)" = "5" ] || { echo "expected 5"; exit 1; }
status=0; err=$("$here/sum.sh" 2>&1 >/dev/null) || status=$?
[ "$status" = "2" ] || { echo "expected exit 2 with no arguments, got $status"; exit 1; }
case "$err" in usage:*) ;; *) echo "expected a usage line on stderr, got: $err"; exit 1 ;; esac
echo "sandbox tests passed"

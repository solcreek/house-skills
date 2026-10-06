#!/usr/bin/env bash
set -euo pipefail
here=$(cd "$(dirname "$0")" && pwd)
[ "$("$here/sum.sh" 1 2)" = "3" ] || { echo "expected 1+2=3, got $("$here/sum.sh" 1 2)"; exit 1; }
[ "$("$here/sum.sh" 5)" = "5" ] || { echo "expected 5"; exit 1; }
echo "sandbox tests passed"

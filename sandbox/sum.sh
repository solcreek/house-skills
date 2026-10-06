#!/usr/bin/env bash
# sum.sh N... — prints the sum of its integer arguments
set -euo pipefail
if [ $# -eq 0 ]; then
  echo "usage: sum.sh N..." >&2
  exit 2
fi
total=0
for n in "$@"; do
  total=$((total + n))
done
echo "$total"

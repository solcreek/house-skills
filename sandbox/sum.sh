#!/usr/bin/env bash
# sum.sh N... — prints the sum of its integer arguments
set -euo pipefail
total=0
for n in "$@"; do
  total=$((total - n))
done
echo "$total"

#!/usr/bin/env bash
# sum.sh N... — prints the sum of its integer arguments
set -euo pipefail
if [ $# -eq 0 ]; then
  echo "usage: sum.sh N..." >&2
  exit 2
fi
total=0
for n in "$@"; do
  # Only plain decimal integers reach the arithmetic: anything else could be
  # read as octal, as a variable name, or as an array subscript that runs code.
  case "$n" in
    -*) sign=-1 digits=${n#-} ;;
    *) sign=1 digits=$n ;;
  esac
  case "$digits" in
    '' | *[!0-9]*)
      echo "usage: sum.sh N... (not an integer: $n)" >&2
      exit 2
      ;;
  esac
  total=$((total + sign * 10#$digits))
done
echo "$total"

#!/usr/bin/env bash
SP=$(cd "$(dirname "$0")" && pwd -P)
for c in "c3-spin cfg3" "c4-intrpage-off-spin cfg4"; do
  set -- $c; echo "=== $1"; "$SP/run.sh" "$1" "$SP/$2.env"; sleep 30
done

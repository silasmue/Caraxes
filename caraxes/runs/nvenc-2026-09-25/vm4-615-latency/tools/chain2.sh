#!/usr/bin/env bash
SP=$(cd "$(dirname "$0")" && pwd -P)
while [[ -e /proc/$1 ]]; do sleep 5; done
sleep 30
"$SP/run.sh" c5-baseline-rearm-trapped "$SP/cfg5.env"; sleep 30
"$SP/run.sh" c6-baseline-repeat "$SP/cfg1.env"

# SPDX-License-Identifier: MIT
# Sourced by limits-check.sh (vm8).
# mig-3: a boot with MIG off at load (GH100: resetless MIG is GSP-RM runtime state, off at
# every GSP boot; mig/gh100-6 kept it and failed), -mig 1, every GPU-instance profile once (create, list,
# destroy), a GI without compute instances, a CI on it by profile, the second GI the
# rung's one GR engine refuses, the teardown and -mig 0.
Q="--query-gpu=mig.mode.current,mig.mode.pending --format=csv"
step q-mig $SMI $Q
step mig1 $SMI -mig 1
step q-mig-on $SMI $Q
step lgip $SMI mig -lgip
for p in 0 5 9 14 20; do
    step "cgi-$p" $SMI mig -cgi $p
    step "lgi-$p" $SMI mig -lgi
    step "dgi-$p" $SMI mig -dgi
done
step cgi-19 $SMI mig -cgi 19
step cgi-19-second $SMI mig -cgi 19
step lgi $SMI mig -lgi
step lcip $SMI mig -lcip
step cci $SMI mig -cci 0 -gi 7
step lci $SMI mig -lci
step list $SMI -L
step dci $SMI mig -dci
step dgi $SMI mig -dgi
step mig0 $SMI -mig 0
step q-mig-end $SMI $Q

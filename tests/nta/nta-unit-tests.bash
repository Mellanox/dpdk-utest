#!/bin/bash

# nta-unit-tests.bash --hosts hosts-file [-s|-v]
#
# Check the `nta-hosts-template.yaml` for NTA hosts file format.
#
# `-s` activates silent mode
# `-v` activates verbose mode
#
# In case of an error in the silent mode the script continues tests execution.
# Otherwise the script exits.


NIC_TESTS='
tests/nta/indirect/indirect-ct_w_cfg.yaml
tests/nta/indirect/indirect-ct.yaml
tests/nta/queue/test-hws2-nic-queue-3-dv1.yaml
tests/nta/queue/test-hws2-nic-queue-3-dv2.yaml
tests/nta/fdb/nta-fdb-cmd.yaml
tests/nta/fdb/nta-fdb-cmd_w_cfg.yaml
tests/nta/fdb/nta-fdb-action-split-queue.yaml
tests/nta/fdb/nta-fdb-action-split-rss.yaml
tests/nta/rss/nta-rss-simple.yaml
tests/nta/rss/nta-indirect.yaml
tests/nta/rss/nta-rss-expand.yaml
tests/nta/meter/test-hws2-nic-meter_mark_no_cfg-3-dv2.yaml
tests/nta/meter/test-hws2-nic-meter_mark-3-dv2.yaml
tests/nta/encap_mh/test-hws2-nic-raw_encap-dv2.yaml
tests/nta/encap_mh/test-hws2-nic-mh2-dv2.yaml
tests/nta/encap_mh/test-hws2-nic-vxlan_encap-dv2.yaml
tests/nta/encap_mh/test-hws2-nic-nvgre_encap-dv2.yaml
tests/nta/encap_mh/test-hws2-nic-nvgre_decap-dv2.yaml
tests/nta/encap_mh/test-hws2-nic-vxlan_decap-dv2.yaml
tests/nta/encap_mh/test-hws2-nic-mh1-dv2.yaml
tests/nta/encap_mh/test-hws2-nic-raw_decap-dv2.yaml
tests/nta/count_age/test-hws2-nic-age_cfg-3-dv2.yaml
tests/nta/count_age/test-hws2-nic-count_no_cfg-3-dv2.yaml
tests/nta/count_age/test-hws2-nic-count_cfg-3-dv2.yaml
tests/nta/count_age/test-hws2-nic-age_no_cfg-3-dv2.yaml
tests/nta/rss/nta-rss-ip6.yaml
tests/nta/hybrid/test-hws2-nic-count_cfg-3-dv2.yaml
'
opt_silent='no'
opt_verbose='no'

while test $# -ne 0; do
case $1 in

'--hosts')
    hosts_file="$2"
    shift; shift
    ;;
'-s')
    opt_silent='yes'
    shift;
    ;;
'-v')
    opt_verbose='yes'
    shift;
    ;;
*)
    echo "invalid option: \"$1\""
    exit 255
esac
done

if test '@'"$hosts_file" = '@'; then
echo "error: no hosts file"
exit 255
fi

options=
test "$opt_silent" = 'yes' && options+='-s'
test "$opt_verbose" = 'yes' && options+='-v'

function run_test () {

commands=$1
hosts=$2
shift; shift;

echo -n "test: $(basename $commands) "
target/release/utest --commands $commands --hosts $hosts $*
if test $? -eq 0; then
echo "OK"
else
echo "FAILED"
echo "cargo run --release -- \\
--hosts $hosts \\
--commands $commands $*"
test ! $opt_silent = 'yes' && exit 255
fi
}

echo -n "Build release ..."
cargo build --release
test $? -ne 0 && exit 255

if test  0; then
for f in $NIC_TESTS; do
    run_test $f "$hosts_file" $options
done 
fi

#!/bin/bash

# unit-tests.bash --hosts hosts-file [-s|-v]
#
# Check the `hosts-template.yaml` for hosts file format.
#
# `-s` activates silent mode
# `-v` activates verbose mode
#
# In case of an error in the silent mode the script continues tests execution.
# Otherwise the script exits.


NIC_TESTS='
tests/sync_HWS/items/extensions/ipv6_ext_no_field_match.yaml
tests/sync_HWS/items/extensions/ipv6_ext.yaml
tests/sync_HWS/items/extensions/nsh.yaml
tests/sync_HWS/items/tunnels/UDP_tunnels/geneve_opt.yaml
tests/sync_HWS/items/tunnels/UDP_tunnels/ib_bth.yaml
tests/sync_HWS/items/tunnels/UDP_tunnels/mpls_o_udp.yaml
tests/sync_HWS/items/tunnels/UDP_tunnels/mpls_o_gre.yaml
tests/sync_HWS/items/flex/flex_rx_no_root.yaml
tests/sync_HWS/actions/edit_packet/vlan_push_set.yaml
tests/sync_HWS/actions/edit_packet/vlan_pop_push.yaml
tests/sync_HWS/actions/edit_packet/indirect_raw_encap_decap.yaml
tests/sync_HWS/actions/edit_packet/raw_encap_gre.yaml
tests/sync_HWS/actions/edit_packet/nvgre_decap.yaml
tests/sync_HWS/actions/edit_packet/vxlan_encap.yaml
tests/sync_HWS/actions/edit_packet/vxlan_decap.yaml
tests/sync_HWS/actions/edit_packet/raw_encap.yaml
tests/sync_HWS/actions/edit_packet/raw_decap.yaml
tests/sync_HWS/actions/edit_packet/nvgre_encap.yaml
tests/sync_HWS/actions/edit_packet/modify_header_src_value.yaml
tests/sync_HWS/actions/edit_packet/modify_header_src_field.yaml
tests/sync_HWS/actions/monitor_diagnostic/count_w_cfg.yaml
tests/sync_HWS/actions/monitor_diagnostic/fdb_cmd_w_cfg.yaml
tests/sync_HWS/actions/monitor_diagnostic/ct_indirect.yaml
tests/sync_HWS/actions/monitor_diagnostic/fdb_cmd_no_cfg.yaml
tests/sync_HWS/actions/monitor_diagnostic/meter_mark_no_cfg.yaml
tests/sync_HWS/actions/monitor_diagnostic/meter_mark_w_cfg.yaml
tests/sync_HWS/actions/monitor_diagnostic/indirect_ct_w_cfg.yaml
tests/sync_HWS/actions/monitor_diagnostic/indirect_ct_no_cfg.yaml
tests/sync_HWS/actions/monitor_diagnostic/count_no_cfg.yaml
tests/sync_HWS/actions/monitor_diagnostic/age_no_cfg.yaml
tests/sync_HWS/actions/monitor_diagnostic/age_w_cfg.yaml
tests/sync_HWS/actions/fate/fdb_action_split_rss.yaml
tests/sync_HWS/actions/fate/fdb_action_split_queue.yaml
tests/sync_HWS/actions/fate/rss_ip6_indirect.yaml
tests/sync_HWS/actions/fate/rss_ip6.yaml
tests/sync_HWS/actions/fate/rss_simple.yaml
tests/sync_HWS/actions/fate/rss_expand.yaml
tests/sync_HWS/actions/fate/indirect.yaml
tests/sync_HWS/actions/fate/queue.yaml
tests/sync_HWS/actions/fate/queue_rss.yaml
tests/sync_HWS/other/port_affinity_match.yaml
tests/sync_HWS/other/hybrid/count.yaml
tests/sync_HWS/other/cross_numa/testpmd_FDB.yaml
tests/sync_HWS/other/cross_numa/testpmd_basic.yaml
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

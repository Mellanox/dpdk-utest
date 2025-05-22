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
tests/flow_sync/actions/fate/fdb_action_split_queue.yaml
tests/flow_sync/actions/fate/fdb_action_split_rss.yaml
tests/flow_sync/actions/fate/indirect.yaml
tests/flow_sync/actions/fate/queue_rss.yaml
tests/flow_sync/actions/fate/queue.yaml
tests/flow_sync/actions/fate/rss_expand.yaml
tests/flow_sync/actions/fate/rss_ip6_indirect.yaml
tests/flow_sync/actions/fate/rss_ip6.yaml
tests/flow_sync/actions/fate/rss_simple.yaml
tests/flow_sync/actions/monitor_diagnostic/age/age_no_cfg.yaml
tests/flow_sync/actions/monitor_diagnostic/age/age_w_cfg.yaml
tests/flow_sync/actions/monitor_diagnostic/count/count_no_cfg.yaml
tests/flow_sync/actions/monitor_diagnostic/count/count_w_cfg.yaml
tests/flow_sync/actions/monitor_diagnostic/count/fdb_cmd_no_cfg.yaml
tests/flow_sync/actions/monitor_diagnostic/count/fdb_cmd_w_cfg.yaml
tests/flow_sync/actions/monitor_diagnostic/CT/ct_indirect.yaml
tests/flow_sync/actions/monitor_diagnostic/CT/indirect_ct_no_cfg.yaml
tests/flow_sync/actions/monitor_diagnostic/CT/indirect_ct_w_cfg.yaml
tests/flow_sync/actions/monitor_diagnostic/meter/meter_mark_no_cfg.yaml
tests/flow_sync/actions/monitor_diagnostic/meter/meter_mark_w_cfg.yaml
tests/flow_sync/actions/packet_reformat/indirect_raw_encap_decap.yaml
tests/flow_sync/actions/packet_reformat/modify_header_src_field.yaml
tests/flow_sync/actions/packet_reformat/modify_header_src_value.yaml
tests/flow_sync/actions/packet_reformat/nvgre_decap.yaml
tests/flow_sync/actions/packet_reformat/nvgre_encap.yaml
tests/flow_sync/actions/packet_reformat/raw_decap.yaml
tests/flow_sync/actions/packet_reformat/raw_encap_gre.yaml
tests/flow_sync/actions/packet_reformat/raw_encap.yaml
tests/flow_sync/actions/packet_reformat/vlan_pop_push.yaml
tests/flow_sync/actions/packet_reformat/vlan_push_set.yaml
tests/flow_sync/actions/packet_reformat/vxlan_decap.yaml
tests/flow_sync/actions/packet_reformat/vxlan_encap.yaml
tests/flow_sync/items/classification_and_integrity/port_affinity_match.yaml
tests/flow_sync/items/classification_and_integrity/tx_queue_item.yaml
tests/flow_sync/items/flex/flex_rx_no_root.yaml
tests/flow_sync/items/IP/ipv6_ext_no_field_match.yaml
tests/flow_sync/items/IP/ipv6_ext.yaml
tests/flow_sync/items/MPLS/mpls_o_gre.yaml
tests/flow_sync/items/MPLS/mpls_o_udp.yaml
tests/flow_sync/items/roce/ib_bth.yaml
tests/flow_sync/items/UDP_tunnels/geneve_opt.yaml
tests/flow_sync/items/UDP_tunnels/vxlan/nsh.yaml
tests/flow_sync/other/cross_numa/testpmd_basic.yaml
tests/flow_sync/other/cross_numa/testpmd_basic_SWS.yaml
tests/flow_sync/other/cross_numa/testpmd_FDB.yaml
tests/flow_sync/other/cross_numa/testpmd_FDB_SWS.yaml
tests/flow_sync/other/hybrid/count.yaml
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

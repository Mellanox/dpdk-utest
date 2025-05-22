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
tests/flow_template_async/actions/fate/customized_rss_hash.yaml
tests/flow_template_async/actions/fate/fdb_esw_mgr.yaml
tests/flow_template_async/actions/fate/fdb_represented_port.yaml
tests/flow_template_async/actions/fate/jump_table_index_action.yaml
tests/flow_template_async/actions/fate/rss_basic.yaml
tests/flow_template_async/actions/monitor_diagnostic/CT/ct.yaml
tests/flow_template_async/actions/monitor_diagnostic/meter/meter_mark.yaml
tests/flow_template_async/actions/monitor_diagnostic/mirror/mirror_16_fdb_ingress_no_reformat.yaml
tests/flow_template_async/actions/monitor_diagnostic/mirror/mirror_fdb_egress_raw_encap_no_jump.yaml
tests/flow_template_async/actions/monitor_diagnostic/mirror/mirror_fdb_egress_raw_encap.yaml
tests/flow_template_async/actions/monitor_diagnostic/mirror/mirror_fdb_ingress_no_reformat_no_jump.yaml
tests/flow_template_async/actions/monitor_diagnostic/mirror/mirror_fdb_ingress_no_reformat.yaml
tests/flow_template_async/actions/monitor_diagnostic/mirror/mirror_multi_esw_egress_raw_encap_no_jump.yaml
tests/flow_template_async/actions/monitor_diagnostic/mirror/mirror_nic_rx_mirror_no_jump.yaml
tests/flow_template_async/actions/monitor_diagnostic/mirror/mirror_nic_rx_mirror.yaml
tests/flow_template_async/actions/monitor_diagnostic/mirror/mirror_quota_fdb.yaml
tests/flow_template_async/actions/monitor_diagnostic/quota/quota_egress.yaml
tests/flow_template_async/actions/monitor_diagnostic/quota/quota_fdb_packet.yaml
tests/flow_template_async/actions/monitor_diagnostic/quota/quota_fdb.yaml
tests/flow_template_async/actions/monitor_diagnostic/quota/quota_shared.yaml
tests/flow_template_async/actions/monitor_diagnostic/quota/quota.yaml
tests/flow_template_async/actions/packet_reformat/fdb_raw_decap.yaml
tests/flow_template_async/actions/packet_reformat/modify_vlan_id_to_tag.yaml
tests/flow_template_async/actions/packet_reformat/nat64.yaml
tests/flow_template_async/actions/packet_reformat/vxlan_modify_last_rsvd.yaml
tests/flow_template_async/items/classification_and_integrity/ptype_l4_tcp.yaml
tests/flow_template_async/items/compare/esp_seq_to_meta.yaml
tests/flow_template_async/items/compare/esp_seq_to_value.yaml
tests/flow_template_async/items/compare/meta_to_value.yaml
tests/flow_template_async/items/compare/random_to_value.yaml
tests/flow_template_async/items/compare/tag_to_value.yaml
tests/flow_template_async/items/crypto/esp.yaml
tests/flow_template_async/items/flex/flex.yaml
tests/flow_template_async/items/IP/basic_ipv4.yaml
tests/flow_template_async/items/IP/basic_ipv6.yaml
tests/flow_template_async/items/IP/icmp6_reply.yaml
tests/flow_template_async/items/IP/icmp6_request.yaml
tests/flow_template_async/items/IP/icmp6.yaml
tests/flow_template_async/items/IP/icmp.yaml
tests/flow_template_async/items/IP/ipv4_range.yaml
tests/flow_template_async/items/IP/ipv6_ext_push_remove.yaml
tests/flow_template_async/items/IP/ipv6_ext.yaml
tests/flow_template_async/items/MPLS/mpls_o_gre.yaml
tests/flow_template_async/items/MPLS/mpls_o_udp.yaml
tests/flow_template_async/items/random/random.yaml
tests/flow_template_async/items/registers/metadata.yaml
tests/flow_template_async/items/registers/tag.yaml
tests/flow_template_async/items/roce/ib_bth.yaml
tests/flow_template_async/items/UDP/basic_relaxed_udp_drop.yaml
tests/flow_template_async/items/UDP/basic_udp_drop.yaml
tests/flow_template_async/items/UDP_tunnels/geneve_options.yaml
tests/flow_template_async/items/UDP_tunnels/gtp/gtp-psc.yaml
tests/flow_template_async/items/UDP_tunnels/gtp/gtp.yaml
tests/flow_template_async/items/UDP_tunnels/vxlan/vxlan_gbp.yaml
tests/flow_template_async/items/UDP_tunnels/vxlan/vxlan_non_relaxed.yaml
tests/flow_template_async/items/UDP_tunnels/vxlan/vxlan.yaml
tests/flow_template_async/items/vlan/qinq_relaxed.yaml
tests/flow_template_async/items/vlan/vlan_not_relaxed.yaml
tests/flow_template_async/items/vlan/vlan.yaml
tests/flow_template_async/other/index_flow_insertion/indexed_based_insertion_flow_rule.yaml
tests/flow_template_async/other/table_resize/table_resize_api.yaml
tests/flow_template_async/other/update_flow/update_rule.yaml
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

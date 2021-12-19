#! /usr/bin/env python3

#
# sudo PYTHONPATH=$PWD/../core ./utest_example.py \
# --config=example.conf --pkt-udp
# @see test.out for the test output
#

from cli import Cli
from rcmd import RCmdError
from testpmd import TestPMD
from testpmd_utest import UTest, UnitTestMatchError

from scapy.all import *

pkt_udp = Ether() / IPv6() / UDP() / Raw(b'/xff' * 4)
pkt_tcp = Ether() / IPv6() / TCP() / Raw(b'/xaa' * 4)
pkt_icmp = Ether() / ICMP() / Raw(b'/x11' * 4)


class TestPMDExample(UTest):
    def __init__(self, id, testpmd:TestPMD, config:dict):
        UTest.__init__(self, id, testpmd, config)

    def __call__(self, pkt):
        flow = 'flow create 0 ingress pattern eth / ipv6 / udp / end actions mark id 0xfab / rss / end'
        try:
            self.testpmd.execute([['start'], ['set verbose 1'], ['flow flush 0']])
            self.testpmd.flow_create([flow])
        except RCmdError as e:
            logging.info('failed: ' + e.cmd)
            return

        try:
            self.send(pkt, match='FDIR matched ID=0xfab')
        except UnitTestMatchError as e:
            logging.info('no match: ' + repr(pkt))
            return
        else:
            logging.info('match: ' + repr(pkt))


if __name__ == "__main__":
    cli = Cli()
    # add application command line parameters
    cli.cl.add_argument('--pkt-udp', action='store_true')
    cli.cl.add_argument('--pkt-tcp', action='store_true')
    cli.cl.add_argument('--pkt-icmp', action='store_true')
    cli.cl.add_argument('--pkt-all', action='store_true')
    cli.parse_args()
    cli.show_config()

    try:
        testpmd = TestPMD(cli.config)
    except Exception as e:
        logging.error("TestPMD failed: ", e.__class__, "occurred.")
        exit(-1)

    if cli.args.pkt_udp or cli.args.pkt_all:
        ut = TestPMDExample('udp', testpmd, cli.config)
        ut(pkt_udp)
    if cli.args.pkt_tcp or cli.args.pkt_all:
        ut = TestPMDExample('tcp', testpmd, cli.config)
        ut(pkt_tcp)
    if cli.args.pkt_icmp or cli.args.pkt_all:
        ut = TestPMDExample('icmp', testpmd, cli.config)
        ut(pkt_icmp)

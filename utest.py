#! /usr/bin/env python3

###
# ./utest.py  --config config.yaml --test test.yaml
###

from conf import TestConf
from rcmd import *
import sys

# sys.tracebacklimit = 0
utest_logger = logging.getLogger('unit-tests-logger')

class Agents:
    testpmd = None
    tg = None
    vm = None

    def __init__(self, conf:dict):
        self.testpmd = TestPMD(conf.test, conf.data)
        if 'tg' in conf.data:
            self.tg = Scapy(conf.data['tg'], 'TG')
        if 'vm' in conf.data:
            self.vm = Scapy(conf.data['vm'], 'VM')

    def close(self):
        for obj in [ self.testpmd, self.tg, self.vm ]:
            if obj is not None: obj.close()


def do_phase(agents:Agents, phase:dict):
    repeat = phase['repeat'] if 'repeat' in phase else 1
    for i in range(0, repeat):
        utest_logger.info("#### PHASE: " + phase['name'] + ' ######')
        for key in phase.keys():
            # first, run commands in order specified by the phase
            if key == 'pmd': agents.testpmd.execute(phase['pmd'])
            if key == 'tg': agents.tg.execute(phase['tg'])
            if key == 'vm': agents.vm.execute(phase['vm'])
            if key == 'result':
                result = phase['result']
                if 'pmd' in result: agents.testpmd.match(result['pmd'])
                if 'tg' in result: agents.tg.match(result['tg'])
                if 'vm' in result: agents.vm.match(result['vm'])


if __name__ == "__main__":
    conf = TestConf()
    if conf.args.test is None: exit(0)

    try:
        agents = Agents(conf)

        for flow in conf.test['flow']:
            for i in range(0, flow['repeat']):
                for phase in flow['phases']: do_phase(agents, phase)

        utest_logger.info('=== TEST COMPLETED')
        # must be called explicitly to prevent paramiko crash - known issue
        agents.close()

    except Exception as e:
        utest_logger.error("Test failed: ", e.__class__)


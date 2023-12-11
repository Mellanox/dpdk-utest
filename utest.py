#! /usr/bin/env python3

import sys
import logging
from remote_ops import *
from remote_cmd import *
from remote_conf import UtestData, UtestCL

utest_logger = logging.getLogger('unit-tests-logger')

def do_phase(agents:dict, phase:dict):
    name = phase.pop('name', '====')
    repeat = phase.pop('repeat', 1)
    result = phase.pop('result', None)

    for i in range(0, repeat):
        utest_logger.info("#### PHASE: " + name + ' ######')
        for app_tag, cmd in phase.items(): agents[app_tag].execute(cmd)
        if not result is None:
            for app_tag, pattern in result.items(): agents[app_tag].match(pattern)


if __name__ == "__main__":
    cl = UtestCL()
    data = UtestData(cl)

    # setup logger
    if cl.args.verbose:
        log_format = '[%(module)s] %(levelname)s %(message)s'
    else:
        log_format = '%(message)s'
        sys.tracebacklimit = 0
    log_level = logging.INFO if not cl.args.verbose else logging.DEBUG
    utest_logger.setLevel(log_level)
    log_handler = logging.StreamHandler()
    log_handler.setLevel(log_level)
    log_handler.setFormatter(logging.Formatter(log_format))
    utest_logger.addHandler(log_handler)

    # import test commands
    if cl.args.commands is None: exit(0)
    data.cmds = data.import_yaml(cl.args.commands)
    if cl.args.show is True:
        data.show_commands()
        exit(0)

    if cl.args.hosts is None: exit(0)
    data.conf = data.import_yaml(cl.args.hosts)

    data.interfaces = data.conf.pop('interfaces', {})
    if not cl.args.fast:
        data.remove_old_netconfig(cl.args.hosts)
        data.configure()
        data.update_config_file(cl.args.hosts, data.interfaces)

    agents = {}
    for app_tag in data.cmds.keys():
        if 'agent' in data.cmds[app_tag]:
            match data.cmds[app_tag]['agent']:
                case 'testpmd': agents[app_tag] = TestPMD(data, app_tag)
                case 'scapy': agents[app_tag] = Scapy(data, app_tag)
                case _:
                    utest_logger.err('Invalid application tag: ' + app_tag)
                    exit(-1)

    try:
        for item in data.cmds['flow']:
            repeat = item['repeat'] if 'repeat' in item else 1
            for i in range(0, repeat):
                for phase in item['phases']: do_phase(agents, phase.copy())
        utest_logger.info('TEST COMPLETED')
    except Exception as e:
        utest_logger.error("Test failed: ", str(e.cmd))

    for a in agents.values(): a.close()
    exit(0)





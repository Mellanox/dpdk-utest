#! /usr/bin/env python3

import yaml
import argparse
import logging
import re

from setup_host import *

utest_logger = logging.getLogger('unit-tests-logger')

class CliError(Exception):
    def __init__(self):
        Exception.__init__(self)


class TestConf:
    data = {}
    test = {}

    def __init__(self):
        self.cl = argparse.ArgumentParser()
        # core environment flags
        self.cl.add_argument('--config', type=str, required=True,
                             help='configuration file')
        self.cl.add_argument('--test', type=str, required=True,
                             help='test commands file')
        self.cl.add_argument('--setup-hw', type=str,
                             help='set DUT MST port')
        self.cl.add_argument('--show', action='store_true',
                             help='show test commands')
        self.cl.add_argument('--dut-fw-reset', action='store_true',
                             help='reset DUT FW')
        self.cl.add_argument('-v', '--verbose', action='store_true',
                             help='add debug logs')
        self.parse_args()
        # execution flags

    def import_yaml(self, filename:str):
        with open(filename, 'r') as f:
            data = yaml.load(f, Loader=yaml.FullLoader)
        f.close()
        return data

    def update_config_file(self):
        interfaces = {'dut':self.data['dut']['interfaces']}
        if 'tg' in self.data.keys():
            interfaces['tg'] = self.data['tg']['interfaces']
        if 'vm' in self.data.keys():
            interfaces['vm'] = self.data['vm']['interfaces']
        with open(self.args.config, 'a') as f:
            f.write('\n\n### Interfaces START\n')
            yaml.dump({'interfaces': interfaces}, f)
            f.write('### Interfaces END\n')
        f.close()

    def show_phase(self, phase:dict):
        pmd = ''
        tg = ''
        vm=''
        if 'pmd' in phase:
            for cmd in phase['pmd']:
                pmd += cmd['command']
        if 'tg' in phase:
            tg = phase['tg']
        if 'vm' in phase:
            vm = phase['vm']
        return pmd,tg,vm


    def validate_pmd_command(self, phase:dict):
        name = phase['name']
        if not isinstance(phase['pmd'], list):
            utest_logger.error(f'[{name}]: bad format: pmd: [...]')
            exit(-1)
        for cmd in phase['pmd']:
            if not isinstance(cmd, dict):
                utest_logger.error(f'[{name}]: bad format: pmd: [{dict}...]')
                exit(-1)
            keys = cmd.keys()
            for tag in ['command', 'result']:
                if not tag in keys:
                    utest_logger.error(f'[{name}]: bad pmd format: missing {tag}')
                    exit(-1)
                if tag == 'command' and not isinstance(cmd[tag], str):
                    utest_logger.error(f'[{name}]: bad pmd format: command is string')
                    exit(-1)
                if tag == 'result':
                    if not isinstance(cmd[tag], str) and \
                            not isinstance(cmd[tag], type(None)):
                        utest_logger.error(f'[{name}]: bad pmd format: '+
                                     'result is string or None')
                        exit(-1)


    def validate_phase(self, phases:list):
        for phase in phases:
            if not isinstance(phase, dict):
                utest_logger.error('bad phase format: not a dict type')
                exit(-1)
            keys = phase.keys()
            if not 'name' in keys:
                utest_logger.error('bad phase format: no "name" tag')
                exit(-1)
            name = phase['name']
            for key in keys:
                if key == 'pmd': self.validate_pmd_command(phase)
                if key in [ 'tg', 'vm', 'result' ]:
                    if not isinstance(key, str):
                        utest_logger.error(f'[{name}]: bad format: ' + key)
                        exit(-1)
                if key == 'result':
                    if not isinstance(phase['result'], dict):
                        utest_logger.error(f'[{name}]: bad result format')
                        exit(-1)

    def validate_flow(self):
        for flow in self.test['flow']:
            if not isinstance(flow, dict):
                utest_logger.error('bad flow format: ' +
                              '{phases: [...], repeat: <num> }} ')
                exit(-1)
            if not 'phases' in flow or not 'repeat' in flow:
                utest_logger.error('bad flow format: ' +
                              '{phases: [...], repeat: <num> }} ')
                exit(-1)
            if not isinstance(flow['phases'], list):
               utest_logger.error('bad flow format: ' +
                             '{phases: [...], repeat: <num> }} ')
               exit(-1)
            self.validate_phase(flow['phases'])


    def validate_test(self):
        utest_logger.info('validating test script ...')
        for tag in [ 'prog', 'flow' ]:
            if not tag in self.test:
                utest_logger.error('bad test format: ' + f'no "{tag}" tag')
                exit(-1)
        if not isinstance(self.test['flow'], list):
            utest_logger.error('bad flow format: ' + 'flow: [ <phase1> ... ] ')
            exit(-1)
        self.validate_flow()
        utest_logger.info('test script is OK')


    def parse_args(self):
        self.args = self.cl.parse_args()
        if self.args.verbose:
            log_format = '[%(module)s] %(levelname)s %(message)s'
        else :
            log_format = '%(message)s'
        log_level = logging.INFO if not self.args.verbose else logging.DEBUG
        utest_logger.setLevel(log_level)
        log_handler = logging.StreamHandler()
        log_handler.setLevel(log_level)
        log_handler.setFormatter(logging.Formatter(log_format))
        utest_logger.addHandler(log_handler)

        # utest_logger.basicConfig(level=log_level, format=log_format)
        self.test = self.import_yaml(self.args.test)
        self.validate_test()
        if self.args.show is True:
            pmd = ''
            tg = ''
            vm=''
            for flow in self.test['flow']:
                for phase in flow['phases']:
                    p, t, v = self.show_phase(phase)
                    pmd +=p
                    tg += t
                    vm += v
            print('############### PMD:\n' + self.test['prog'] + '\n\n' + pmd)
            print('############### TG:\n' + tg)
            print('############### VM:\n' + vm)
            exit(0)
        self.data = self.import_yaml(self.args.config)

        cmdline = self.test['prog']

        dut = self.data['dut']
        setup = self.test['setup']
        if self.args.setup_hw is not None:
            setup['hw'] = self.args.setup_hw
        flags = DUT_FW_RESET if self.args.dut_fw_reset else 0
        dut['mst_dev'] = setup_dut(setup, dut, flags=flags)
        utest_logger.debug('mst device: ' + dut['mst_dev'])
        if 'interfaces' not in self.data.keys():
            utest_logger.debug('resolve interfaces')
            dut['interfaces'] = dut_interfaces(dut, dut['mst_dev'])
            utest_logger.debug('DUT interfaces: ' + str(dut['interfaces']))

            if 'tg' in self.data.keys():
                tg = self.data['tg']
                tg['interfaces'] = host_interfaces(tg, dut['mst_dev'])
                utest_logger.debug('TG interfaces: ' + str(tg['interfaces']))
                self.data['tg']['interfaces'] = tg['interfaces']

            if 'vm' in self.data.keys():
                vm = self.data['vm']
                vm['interfaces'] = host_interfaces(vm, dut['mst_dev'])
                utest_logger.debug('VM interfaces: ' + str(vm['interfaces']))
                self.data['vm']['interfaces'] = vm['interfaces']

                self.update_config_file()
        else:
            utest_logger.debug('reuse interfaces')
            self.data['dut']['interfaces'] = self.data['interfaces']['dut']
            if 'tg' in self.data.keys():
                self.data['tg']['interfaces'] = self.data['interfaces']['tg']
            if 'vm' in self.data.keys():
                self.data['vm']['interfaces'] = self.data['interfaces']['vm']

        for port in dut['interfaces'].keys():
            cmdline = re.sub(port, dut['interfaces'][port], cmdline)
        utest_logger.debug('cmdline:' + cmdline)
        self.test['prog'] = cmdline


#! /usr/bin/env python3

import yaml
import argparse
import logging
import re


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
        self.cl.add_argument('--show', action='store_true',
                             help='show test commands')
        self.cl.add_argument('-v', '--verbose', action='store_true',
                             help='add debug logs')
        self.parse_args()
        # execution flags

    def import_yaml(self, filename:str):
        with open(filename, 'r') as f:
            data = yaml.load(f, Loader=yaml.FullLoader)
        f.close()
        return data

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
            logging.error(f'[{name}]: bad format: pmd: [...]')
            exit(-1)
        for cmd in phase['pmd']:
            if not isinstance(cmd, dict):
                logging.error(f'[{name}]: bad format: pmd: [{dict}...]')
                exit(-1)
            keys = cmd.keys()
            for tag in ['command', 'result']:
                if not tag in keys:
                    logging.error(f'[{name}]: bad pmd format: missing {tag}')
                    exit(-1)
                if tag == 'command' and not isinstance(cmd[tag], str):
                    logging.error(f'[{name}]: bad pmd format: command is string')
                    exit(-1)
                if tag == 'result':
                    if not isinstance(cmd[tag], str) and \
                            not isinstance(cmd[tag], type(None)):
                        logging.error(f'[{name}]: bad pmd format: '+
                                     'result is string or None')
                        exit(-1)


    def validate_phase(self, phases:list):
        for phase in phases:
            if not isinstance(phase, dict):
                logging.error('bad phase format: not a dict type')
                exit(-1)
            keys = phase.keys();
            if not 'name' in keys:
                logging.error('bad phase format: no "name" tag')
                exit(-1)
            name = phase['name']
            for key in keys:
                if key == 'pmd': self.validate_pmd_command(phase)
                if key in [ 'tg', 'vm', 'result' ]:
                    if not isinstance(key, str):
                        logging.error(f'[{name}]: bad format: ' + key)
                        exit(-1)
                if key == 'result':
                    if not isinstance(phase['result'], dict):
                        logging.error(f'[{name}]: bad result format')
                        exit(-1)

    def validate_flow(self):
        for flow in self.test['flow']:
            if not isinstance(flow, dict):
                logging.error('bad flow format: ' +
                              '{phases: [...], repeat: <num> }} ')
                exit(-1)
            if not 'phases' in flow or not 'repeat' in flow:
                logging.error('bad flow format: ' +
                              '{phases: [...], repeat: <num> }} ')
                exit(-1)
            if not isinstance(flow['phases'], list):
               logging.error('bad flow format: ' +
                             '{phases: [...], repeat: <num> }} ')
               exit(-1)
            self.validate_phase(flow['phases'])


    def validate_test(self):
        for tag in [ 'prog', 'flow' ]:
            if not tag in self.test:
                logging.error('bad test format: ' + f'no "{tag}" tag')
                exit(-1)
        if not isinstance(self.test['flow'], list):
            logging.error('bad flow format: ' + 'flow: [ <phase1> ... ] ')
            exit(-1)
        self.validate_flow()

    def parse_args(self):
        self.args = self.cl.parse_args()
        if self.args.verbose:
            log_format = '[%(module)s] %(levelname)s %(message)s'
        else :
            log_format = '%(message)s'
        log_level = logging.INFO if not self.args.verbose else logging.DEBUG
        logging.basicConfig(level=log_level, format=log_format)
        self.data = self.import_yaml(self.args.config)
        self.test = self.import_yaml(self.args.test)
        self.validate_test()
        for i in range(len(self.data['dut']['ports'])):
            self.test['prog'] = \
                re.sub('PORT_' + str(i), self.data['dut']['ports'][i],
                       self.test['prog'])
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

#! /usr/bin/env python3

import argparse
import logging
import re


class CliError(Exception):
    def __init__(self):
        Exception.__init__(self)


class Cli:
    DEFAULT_CONFIG_FILE = 'ENV'
    config = {}

    def __init__(self):
        self.cl = argparse.ArgumentParser()
        # core environment flags
        self.cl.add_argument('--config', type=str,
                             help='configuration file')
        self.cl.add_argument('--show-config', action='store_true',
                             help='show active configuration and exit')
        self.cl.add_argument('-v', '--verbose', action='store_true',
                             help='add debug logs')
        # execution flags

    def read_env(self):
        self.config['file'] = self.args.config \
                              if self.args.config is not None \
                              else self.DEFAULT_CONFIG_FILE
        try:
            f = open(self.config['file'], 'r').readlines()
        except Exception as e:
            logging.error('cannot read ' + self.args.config)
            exit(-1)

        l = list(map(lambda e: re.split('[=#\n]', e), f))
        for e in l:
            self.config[e[0]] = e[1]
        if re.search('[\'\"]', self.config['cmd']) is not None:
            self.config['cmd'] = re.split('[\'\"]', self.config['cmd'])[1]

    def parse_args(self):
        self.args = self.cl.parse_args()
        log_format = '[%(module)s] %(levelname)s %(message)s'
        log_level = logging.INFO if not self.args.verbose else logging.DEBUG
        logging.basicConfig(level=log_level, format=log_format)
        self.read_env()

    def show_config(self):
        for key in self.config.keys():
            print(f'{key}={self.config[key]}')
        if self.args.show_config: exit(0)

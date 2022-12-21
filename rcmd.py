#! /usr/bin/env python3

import paramiko
import time, re
import logging
import os

from conf import TestConf

class RCmdError(Exception):
    def __init__(self, cmd):
        self.cmd = cmd


class RCmd:
    ssh = None
    delim = 'Oops '
    name='RCmd> '
    output = ''

    def __init__(self, rhost:str, username:str, password:str, prog:str):
        try:
            self.ssh = paramiko.SSHClient()
            self.ssh.load_system_host_keys()
            self.ssh.set_missing_host_key_policy(paramiko.AutoAddPolicy())
            logging.info(prog)
            self.ssh.connect(rhost, username=username, password=password)
            # force stderr stdout flush
            self.stdin, \
            self.stdout, \
            self.stderr = self.ssh.exec_command(
                '/usr/bin/stdbuf -oL -eL ' + prog)
            self.rdout()
        except Exception as e:
            if self.stdout is not None: print(self.stdout.readlines())
            logging.error("TestPMD failed: ", e.__class__, "occurred.")
            exit(-1)

    # paramiko stdout.readline() can block
    def rdout(self):
        time.sleep(.1)
        self.stdin.write(self.delim + '\n')
        self.stdin.flush()
        time.sleep(.1)
        while True:
            line = self.stdout.readline()
            if len(line) > 1:
                if re.search(self.delim, line) is None:
                    logging.info(self.name + line.strip())
                    self.output += line
                else:
                    break


    def execute(self, command:str):
        self.output = ''
        self.stdin.write(command + '\n')
        self.stdin.flush()
        self.rdout()

    def match(self, expected:str):
        self.rdout()
        pattern = r'{}'.format(expected)
        logging.debug('\n>>>>>\n'+pattern+'\n<<<<<')
        res = re.search(pattern, self.output, re.DOTALL|re.MULTILINE)
        if not res: raise RCmdError('\n=== match failed\nexpected ' + expected
                                    + '\noutput: ' + self.output)

    def close(self):
        self.ssh.close()
        del self.ssh, self.stdin, self.stdout, self.stderr

class TestPMD(RCmd):
    def __init__(self, conf:TestConf):
        self.delim = '###'
        self.name = 'PMD> '

        dut = conf.data['dut']
        testpmd = dut['path'] + '/' + conf.test['prog'] + ' 2> \&1'
        logging.info('TESTPMD> connecting to '
                     + dut['username'] + '@' + dut['host'])
        RCmd.__init__(self, dut['host'], dut['username'], dut['password'],
                      testpmd)
        super().execute('show port summary all')
        self.match('^0\s{1,}([0-9A-F]{2}:){5}[0-9A-F]{2}')

    #
    # PMD commands format: [ command1 ... ]
    #
    # command dictionary format:
    # { 'command': <string>, 'result': <string> | None }
    #
    def execute(self, commands:list):
        for cmd in commands:
            super().execute(cmd['command'])
            if cmd['result'] is not None: self.match(cmd['result'])


class Scapy(RCmd):
    def __init__(self, conf:dict, tag:str):
        self.delim = "'END-OF-INPUT'"
        self.name = tag + '> '

        netdev_names = ('if', 'pf', 'vf', 'rf')

        logging.info(f'{tag}> connecting to ' +
                     conf['username'] + '@' + conf['host'])
        RCmd.__init__(self, conf['host'], conf['username'], conf['password'],
                      'python3 -i -u - 2> \&1')
        for name in netdev_names:
            for i in ( '0', '1'):
                dev = name + i
                if dev in conf.keys():
                    logging.debug(f'{tag}> {dev} = {conf[dev]}')
                    self.execute(dev + ' = \'' + conf[dev] + '\'')

        logging.debug("===================")
        self.execute('from scapy.all import *')
        str = r"(Ether(src='11:22:33:44:55:66', dst='aa:bb:cc:dd:ee:aa')/\
              IP(src='1.1.1.1', dst='2.2.2.2')/\
              UDP(sport=1234, dport=5678)).command()"
        self.execute(str)
        self.match('sport=1234, dport=5678')
        logging.debug("===================")


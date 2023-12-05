#! /usr/bin/env python3

import paramiko
import time, re
import logging
from remote_ops import RemoteOps
from remote_conf import UtestData
from typing import Union
from collections import OrderedDict

utest_logger = logging.getLogger('unit-tests-logger')

class RCmdError(Exception):
    def __init__(self, cmd):
        self.cmd = cmd


class RCmd:
    ssh = None
    delim = 'Oops '
    name='RCmd> '
    output = ''

    # (self, rhost:str, username:str, password:str, prog:str)
    def __init__(self, rhost:str, **kwargs):
        try:
            self.ssh = paramiko.SSHClient()
            self.ssh.load_system_host_keys()
            self.ssh.set_missing_host_key_policy(paramiko.AutoAddPolicy())

            uname = None
            passwd = None
            if 'ssh_key' in kwargs.keys():
                self.ssh.connect(rhost, key_filename=kwargs['ssh_key'])
            else:
                if 'username' in kwargs.keys(): uname = kwargs['username']
                if 'password' in kwargs.keys(): passwd = kwargs['password']
                self.ssh.connect(rhost, username=uname, password=passwd)

            if 'prog' in kwargs.keys():
                prog = kwargs['prog']
            else:
                prog = '/bin/sh'
            utest_logger.info(prog)
            # force stderr stdout flush
            self.stdin, \
            self.stdout, \
            self.stderr = self.ssh.exec_command(
                '/usr/bin/stdbuf -oL -eL ' + prog)
            self.rdout()
        except Exception as e:
            if self.stdout is not None: print(self.stdout.readlines())
            utest_logger.error("TestPMD failed: ", e.__class__, "occurred.")
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
                    utest_logger.info(self.name + line.strip())
                    self.output += line
                else:
                    break


    def execute(self, command:str, **kwargs):
        self.output = ''
        self.stdin.write(command + '\n')
        self.stdin.flush()
        if 'check_proc' in kwargs.keys() and not kwargs['check_proc']():
            utest_logger.info('killer command: ' + command)
            exit(-1)
        self.rdout()

    def match_str(self, expected:str) -> bool:
        pattern = r'{}'.format(expected)
        utest_logger.debug('\n>>>>>\n'+pattern+'\n<<<<<')
        res = re.search(pattern, self.output, re.DOTALL|re.MULTILINE)
        if res is None: return False
        self.output = self.output[res.span()[1]:]
        return True

    def match_dict(self, expected:dict) -> bool:
        for key in expected.keys():
            if key == 'all':
                for p in expected[key]:
                    if not self.match_str(p): return False
                return True
            if key == 'some':
                for p in expected[key]:
                    if self.match_str(p): return True
                return False

    def match(self, expected:Union[str, dict]):
        self.rdout()
        if isinstance(expected, str): verdict = self.match_str(expected)
        elif isinstance (expected, dict): verdict = self.match_dict(expected)
        else: verdict = False
        if not verdict:
            raise RCmdError('\n=== match failed\nexpected ' + str(expected) + '\noutput: ' + self.output)

    def close(self):
        self.ssh.close()
        del self.ssh, self.stdin, self.stdout, self.stderr

class TestPMD(RCmd):
    ctrl = None

    def is_alive(self) -> bool:
        res = re.search('--file-prefix(\s){1,}(\S){1,}', self.cmd)
        if res is None: lock='/run/dpdk/rte/config'
        else: lock = '/run/dpdk/' + res.group(0).split()[1]
        output = self.ctrl.rsh['lslocks']['-u', '-o', 'COMMAND,PATH']()
        res = re.search('^dpdk-testpmd(\s){1,}' + lock, output, re.DOTALL|re.MULTILINE)
        return res is not None

    def __init__(self, data:UtestData, app_id:str):
        self.delim = '###'
        self.name = f'{app_id}> '
        self.cmd = data.cmds[app_id]['cmd']

        host = data.conf[app_id]['host']
        self.ctrl = RemoteOps(host, **data.ssh_params).connect()
        if self.is_alive():
            utest_logger.error(f'{self.name}Enother testpmd process is running')
            exit(-1)


        # sort interface keys according to length to prevent partial match
        pci_map = OrderedDict(sorted(data.interfaces[host]['pci'].items(), reverse=True, key=lambda t: len(t[0])))
        for pci_id, pci in pci_map.items(): self.cmd = re.sub(pci_id, pci, self.cmd)
        cmd = data.conf[app_id]['path'] + '/' + self.cmd + ' 2>&1'
        utest_logger.info(f'{self.name} connecting to ' + host)
        RCmd.__init__(self, host, prog=cmd, **data.ssh_params)
        super().execute('show port summary all')
        self.match('^0\s{1,}([0-9A-F]{2}:){5}[0-9A-F]{2}')

    def close(self):
        self.ctrl.disconnect()
        super().close()

    #
    # PMD commands format: [ command1 ... ]
    #
    # command dictionary format:
    # { 'command': <string>, 'result': <string> | None }
    #
    def execute(self, commands:list):
        if not self.is_alive():
            utest_logger.error(f'{self.name} testpmd is dead')
            exit(-1)
        for cmd in commands:
            super().execute(cmd['command'], check_proc=self.is_alive)
            if cmd['result'] is not None: self.match(cmd['result'])


class Scapy(RCmd):
    def __init__(self, data:UtestData, app_id:str):
        self.delim = "'END-OF-INPUT'"
        self.name = f'{app_id}> '

        host = data.conf[app_id]['host']
        utest_logger.info(f'{self.name} connecting to ' + host)
        RCmd.__init__(self, host, prog='python3 -i -u - 2> \&1', **data.ssh_params)

        for dev_id, netdev in data.interfaces[host]['netdev'].items():
            _log = f'{self.name} {dev_id} = {netdev}'
            utest_logger.debug(_log)
            self.execute(dev_id + ' = \'' + netdev + '\'')

        utest_logger.debug("===================")
        self.execute('from scapy.all import *')
        str = r"(Ether(src='11:22:33:44:55:66', dst='aa:bb:cc:dd:ee:aa')/\
              IP(src='1.1.1.1', dst='2.2.2.2')/\
              UDP(sport=1234, dport=5678)).command()"
        self.execute(str)
        self.match('sport=1234, dport=5678')
        utest_logger.debug("===================")

    def execute(self, command:str):
        utest_logger.info(self.name + command.strip())
        super().execute(command)


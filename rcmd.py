#! /usr/bin/env python3

import paramiko
import time, re
import logging
from remote_ops import RemoteOps

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
        if 'check_proc' in kwargs.keys() and kwargs['check_proc']() is False:
            raise RCmdError('killer command:\n' + command)
        self.rdout()

    def match(self, expected:str):
        self.rdout()
        pattern = r'{}'.format(expected)
        utest_logger.debug('\n>>>>>\n'+pattern+'\n<<<<<')
        res = re.search(pattern, self.output, re.DOTALL|re.MULTILINE)
        if not res: raise RCmdError('\n=== match failed\nexpected ' + expected
                                    + '\noutput: ' + self.output)

    def close(self):
        self.ssh.close()
        del self.ssh, self.stdin, self.stdout, self.stderr

class TestPMD(RCmd):
    ctrl = None

    def __init__(self, test:dict, data:dict):
        self.delim = '###'
        self.name = 'PMD> '

        dut = data['dut']
        self.ctrl = RemoteOps(dut['host'],
                              username=dut['username'],
                              password=dut['password']).connect()

        if self.is_alive() is True:
            utest_logger.warn('Enother testpmd process is running')
            exit(-1)

        testpmd = dut['path'] + '/' + test['prog'] + ' 2> \&1'
        utest_logger.info('TESTPMD> connecting to '
                     + dut['username'] + '@' + dut['host'])
        RCmd.__init__(self, dut['host'],
                      username=dut['username'], password=dut['password'],
                      prog=testpmd)
        super().execute('show port summary all')
        self.match('^0\s{1,}([0-9A-F]{2}:){5}[0-9A-F]{2}')

    def is_alive(self) -> bool:
        output = self.ctrl.rsh['lslocks']['-u', '-o', 'COMMAND,PATH']()
        return re.search('^dpdk-testpmd.*dpdk/rte/config', output,
                         re.DOTALL|re.MULTILINE) is not None

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
        if self.is_alive() is False: raise RCmdError('tespmd is dead')
        for cmd in commands:
            super().execute(cmd['command'], check_proc=self.is_alive)
            if cmd['result'] is not None: self.match(cmd['result'])


class Scapy(RCmd):
    def __init__(self, conf:dict, tag:str):
        self.delim = "'END-OF-INPUT'"
        self.name = tag + '> '

        utest_logger.info(f'{tag}> connecting to ' +
                     conf['username'] + '@' + conf['host'])
        RCmd.__init__(self, conf['host'],
                      username=conf['username'], password=conf['password'],
                      prog='python3 -i -u - 2> \&1')

        interfaces = conf['interfaces']
        for dev in interfaces.keys():
            _log = f'{tag}> {dev} = {interfaces[dev]}'
            utest_logger.debug(_log)
            self.execute(dev + ' = \'' + interfaces[dev] + '\'')

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

class Shell(RCmd):
    def __init__(self, conf):
        self.delim = '###'
        self.name = conf['host'] +'> '
        utest_logger.info('{0} {1} {2}'.format(conf['host'], conf['username'],conf['password']))
        RCmd.__init__(self, conf['host'],
                      username=conf['username'], password=conf['password'])

    def netdev_resolve_mac(self, mac:str):
        commands = """
            result='none none'
            for p in /sys/class/net/*; do
            address="$(cat $p/address)"
            #echo "$address"
            if test ${address} = ${mac}; then
            pci=$(ethtool -i $(basename $p) | awk '/^bus-info/ {print $NF}')
            echo $(basename $p) $pci
            fi
            done
            """

        utest_logger.debug(self.name + 'send command:\n' + f'mac={mac}')
        utest_logger.debug(self.name + 'send command: ' + commands)
        self.stdin.write(f'mac={mac}' + '\n')
        self.stdin.write(commands + '\n')
        utest_logger.info(self.name + 'get result')
        netdev,pci = self.stdout.readline().split()
        utest_logger.debug(self.name + f'mac:{mac} netdev:{netdev} PCI:{pci}')
        return netdev,pci


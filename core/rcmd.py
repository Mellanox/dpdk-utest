#! /usr/bin/env python3

import paramiko
import time, re
import logging
import os


class RCmdError(Exception):
    def __init__(self, cmd):
        self.cmd = cmd


class RCmd:
    DEFAULT_DELIM = '###'

    def __init__(self, config: dict):
        cmd = os.path.join(config['dut_cmd_path'], config['dut_cmd'])
        self.config = config
        self.config['delim'] = config['delim'] \
            if 'delim' in config.keys() \
            else self.DEFAULT_DELIM
        logging.debug('delim: ' + config['delim'])
        try:
            self.ssh = paramiko.SSHClient()
            self.ssh.load_system_host_keys()
            self.ssh.set_missing_host_key_policy(paramiko.AutoAddPolicy())
            logging.info(cmd)
            self.ssh.connect(config['dut'],
                             username=config['dut_username'],
                             password=config['dut_password'])
            # force stderr stdout flush
            self.stdin, \
            self.stdout, \
            self.stderr = self.ssh.exec_command(
                '/usr/bin/stdbuf -oL -eL ' + cmd)
        except Exception as e:
            logging.error("TestPMD failed: ", e.__class__, "occurred.")
            exit(-1)
        self.rdout()

    # paramiko stdout.readline() can block
    def rdout(self):
        out = []
        self.stdin.write(self.config['delim'] + '\n')
        self.stdin.flush()
        time.sleep(.1)
        while True:
            line = self.stdout.readline()
            if len(line) > 1:
                logging.info('DUT> ' + line.strip())
                if re.search(self.config['delim'], line) is None:
                    out.append(line)
                else:
                    break
        return out

    #
    # examples:
    # [[cmd1], [cmd2, out2] ... ]
    def execute(self, commands: list):
        for cmd in commands:
            c = re.sub(r'[\s]+', ' ', cmd[0])
            logging.info('DUT> ' + c)
            self.stdin.write(c + '\n')
            self.stdin.flush()
            time.sleep(.1)
            if len(cmd) == 1 or cmd[1] == '':
                match = cmd[0].split(' ', 1)[0]
            else:
                match = cmd[1]
            logging.info('expected out: ' + match)
            verdict = False
            for line in self.rdout():
                if re.search(match, line) is not None:
                    verdict = True
                    break
            if not verdict:
                raise RCmdError(cmd[0])
                return

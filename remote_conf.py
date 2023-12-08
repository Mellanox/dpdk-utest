#! /usr/bin/env python3

import re
import yaml
import time
import argparse
from remote_ops import *

utest_logger = logging.getLogger('unit-tests-logger')

class UtestCL:
    def __init__(self):
        self.cl = argparse.ArgumentParser()
        # core environment flags
        self.cl.add_argument('--hosts', type=str, required=True,
                             help='hosts configuration file')
        self.cl.add_argument('--commands', type=str, required=True,
                             help='test commands file')
        self.cl.add_argument('--show', action='store_true',
                             help='show test commands')
        self.cl.add_argument('--mt-dev', type=str,
                             help='MT device type')
        self.cl.add_argument('-f', '--fast', action='store_true',
                             help='fast execution, skip DUT configuration')
        self.cl.add_argument('-v', '--verbose', action='store_true',
                             help='add debug logs')
        self.args = self.cl.parse_args()

class UtestData:
    ssh_params = {'username': 'root', 'password': '3tango'}

    def __init__(self, cl:UtestCL):
        self.conf = {}
        self.cmds = {}
        self.remotes = {}
        self.interfaces = {}
        self.mt_dev = None
        self.cl = cl

    def update_config_file(self, filename: str, interfaces: dict):
        with open(filename, 'a') as f:
            yaml.dump({'interfaces': interfaces}, f)
        f.close()

    def remove_old_netconfig(self, filename: str):
        with open(filename, 'r') as f:
            data = f.read()
        f.close()
        if re.search('\ninterfaces:', data) is None: return
        utest_logger.info('remove existing intefaces map')
        old_conf = False
        new_lines = []
        for line in data.split('\n'):
            if re.search('^interfaces:', line) is not None:
                old_conf = True
            elif old_conf and re.search('^\s', line) is not None:
                continue
            else:
                old_conf = False
                new_lines.append(line)
        with open(filename, 'w') as f:
            for l in new_lines: f.write(l + '\n')
        f.close()

    def select_mt_dev(self):
        app = list(self.conf.keys())[0]
        host = self.conf[app]['host']
        mt_db = self.remotes[host]['ops'].dev_db
        self.mt_dev = list(mt_db.keys())[0]

    def connect_remotes(self):
        for host in set(self.conf[app]['host'] for app in self.conf.keys()):
            self.remotes[host] = {'ops': None}
            self.remotes[host]['ops'] = RemoteOps(host, **self.ssh_params).connect()

    def disconnect_remotes(self):
        for remote in self.remotes.values():
            remote['ops'].disconnect()

    def configure(self):
        self.connect_remotes()
        if not self.cl.args.mt_dev is None: self.mt_dev = self.cl.args.mt_dev
        else: self.select_mt_dev()
        self.config_interfaces()
        self.remote_interfaces()
        self.disconnect_remotes()

    def config_interfaces(self):
        for app in self.conf.keys():
            if not app in self.cmds.keys(): continue
            host = self.conf[app]['host']
            if 'setup' in self.cmds[app].keys():
                setup = self.cmds[app]['setup']
                utest_logger.info(f'configure {host} for {app}')
                self.configure_host(self.remotes[host], setup)
    def reset_host_state(self, remote: dict) -> RemoteOps:
        ops = remote['ops']
        host = ops.rhost
        mt_dev = self.mt_dev
        if not ops.cloud_host():
            if not ops.fw_reset(mt_dev):
                _log = f'{ops.rhost}: failed to reset FW mst_dev {mt_dev}'
                utest_logger.error(_log)
                exit(-1)
            return ops
        # FW reset on cloud hosts is unreliable.
        # reboot is faster and saver
        ops.reboot()
        ssh_params = self.ssh_params.copy()
        ssh_params['connect_timeout'] = 30
        for attempt in list(range(0,5)):
            try:
                time.sleep(1 + 0.5 * attempt)
                remote['ops'] = RemoteOps(host, **ssh_params).connect()
            except Exception as e:
                remote['ops'] = None
                pass
        if remote['ops'] is None:
            utest_logger.error(f'Cannot reconnect to {host}')
            exit(-1)
        return remote['ops']


    def configure_host(self, remote: dict, setup: dict):
        ops = self.reset_host_state(remote)
        mt_dev = self.mt_dev
        _log = f'{ops.rhost}: mst_dev {mt_dev}'
        utest_logger.info(_log)
        if 'vf' in setup.keys():
            for port_id, vf_config in enumerate(setup['vf']):
                ops.config_vf(mt_dev, port_id, vf_config)
        for port_id, dom in enumerate(setup['domain']):
            if dom == 'fdb':
                ops.config_fdb(mt_dev, port_id)
        ops.config_huge_pages(1024)

    def host_interfaces(self, remote:dict, interfaces:dict):
        ops = remote['ops']
        mt_dev = self.mt_dev
        mt_db = ops.dev_db
        pci = interfaces['pci']
        netdev = interfaces['netdev']
        for pf_id, pf_pci in enumerate(mt_db[mt_dev]):
            pf_key = f'pci{pf_id}'
            pci[pf_key] = pf_pci
            for vf_id, vf_pci in enumerate(ops.show_vf(mt_dev, pf_id)):
                vf_key = f'{pf_key}vf{vf_id}'
                pci[vf_key] = vf_pci
        for pf_id, pf_pci in enumerate(mt_db[mt_dev]):
            pf_key = f'pf{pf_id}'
            netdev[pf_key] = ops.pci_to_netdev(pf_pci)
            for vf_id, vf_pci in enumerate(ops.show_vf(mt_dev, pf_id)):
                vf_key = f'{pf_key}vf{vf_id}'
                netdev[vf_key] = ops.pci_to_netdev(vf_pci)
            for rep_id, rep in enumerate(ops.show_port_representors(mt_dev, pf_id)):
                rep_key = f'{pf_key}rf{rep_id}'
                netdev[rep_key] = rep

    def remote_interfaces(self):
        for host in set(self.remotes.keys()):
            self.interfaces[host] = {'pci': {}, 'netdev': {}}
            self.host_interfaces(self.remotes[host], self.interfaces[host])

    def import_yaml(self, filename:str) -> dict:
        with open(filename, 'r') as f:
            data = yaml.load(f, Loader=yaml.FullLoader)
        f.close()
        return data

    def show_commands(self):
        agents = { app: conf for app, conf in self.cmds.items() if 'agent' in self.cmds[app] }
        agents = { app: conf for app, conf in sorted(agents.items(), key=lambda x: x[1]['agent'], reverse=True)}
        for app, data in agents.items(): self.show_app_commands(app, data)
        return

    def show_app_commands(self, app:str, data:dict):
        print(f'=========== {app}')

        if 'cmd' in data.keys(): print('COMMAND: ' + data['cmd'] + '\n')

        for item in self.cmds['flow']:
            for phase in item['phases']:
                if not app in phase.keys(): continue
                if isinstance(phase[app], str): print(phase[app])
                elif isinstance(phase[app], list):
                    for cmd in phase[app]: print(cmd['command'])

#! /usr/bin/env python3

import re
import logging
from plumbum import SshMachine

HCA_MAX_PORTS_NUM = 2
PF_MAX_VF_NUM = 8
PCI_ADDR_LEN = len('0000:00:00.0')

utest_logger = logging.getLogger('unit-tests-logger')

class RemoteOps:
    rsh = None
    rhost = None
    user = None
    password = None
    keyfile = None
    dev_db = {}

    def __init__(self, rhost:str, **kwargs):
        self.rhost = rhost
        self.user = kwargs['username'] if 'username' in kwargs.keys() else None
        self.password = kwargs['password'] if 'password' in kwargs.keys() else None
        self.keyfile = kwargs['keyfile'] if 'keyfile' in kwargs.keys() else None

    def sysfs_config(self, sysfs_file:str, token:str):
        self.rsh['sh']['-c', f'echo {token} > {sysfs_file}']()

    def connect(self):
        if self.rsh is not None: return self
        utest_logger.debug('trying to connect to: ' + str(self.rhost))
        try:
            self.rsh = SshMachine(host=self.rhost, user=self.user,
                                  password=self.password, keyfile=self.keyfile,
                                  connect_timeout=3)
        except Exception as e:
            utest_logger.error('connection error: ' + str(type(e)))
            exit(1)
        finally:
            self.mst_status()
            print(self.dev_db)
            return self

    def disconnect(self):
        if self.rsh is not None:
            self.rsh.close()
            self.rsh = None

    def cloud_host(self) -> bool:
        out=self.rsh['ls']['/']()
        return re.search('workspace', out) is not None

    def reboot(self):
        self.rsh['reboot']()
        self.rsh = None

    def fw_reset_cloud_host(self):
        out=''
        fw_reset = self.rsh['/workspace/cloud_tools/cloud_firmware_reset.sh']
        for round in range(0,3):
            try:
                out = fw_reset['--ips', self.rhost]() # blocking
            except Exception as e:
                utest_logger.info('Error: ' + str(type(e)))
                continue

            if re.search('"status": "success"', out) is not None:
                utest_logger.debug('FW reset: OK')
                return True
            else:
                utest_logger.info('FW reset: failed')

        return False

    def fw_reset(self, mt:str) -> bool:
        _log = f'{self.rhost}: reset FW'
        utest_logger.info(_log)
        if self.cloud_host(): res = self.fw_reset_cloud_host()
        else:
            mst_dev = f'/dev/mst/{mt}_pciconf0'
            fw_reset = self.rsh['mlxfwreset']
            fw_reset['-d', f'{mst_dev}', 'r', '-y',  '--level', '3']()
            res = True
        return res

    def fw_version(self) -> str:
        raw = self.rsh['mlxfwmanager']()
        out=re.search('FW\s{1,}(\d{2}.){2}\d{4}', raw)
        return out.group(0).split()[1][3:]

    def mst_status(self) -> dict:
        self.dev_db = {}
        mst = self.rsh['mst']
        mst['restart']()
        mst_output = mst['status', '-v']().split('\n')
        # DEVICE_TYPE    MST                        PCI       RDMA         NET               NUMA
        # ConnectX5    /dev/mst/mt4121_pciconf0    42:00.0   mlx5_4    net-eth1,net-enp66s0f0np0,net-eth01
        for line in mst_output:
            if re.search('/dev/mst/mt', line) is None: continue
            if re.search('mt4103|mt4117', line) is not None: continue
            dev_port = line.split()[1].split('/')[3].split('_')
            dev = dev_port[0]
            raw = dev_port[1][7:]
            if re.search('\.', raw) is None: # mt41686_pciconf0
                port= int(raw)
            else:                            # mt41686_pciconf0.1
                port = int(raw.split('.')[1])
            if dev not in self.dev_db.keys():
                self.dev_db[dev] = [None] * HCA_MAX_PORTS_NUM
            pci = line.split()[2]
            if re.match('\d{4}:\d{2}:\d{2}.\d', pci) is None:
                pci = '0000:' + pci
            self.dev_db[dev][port] = pci
        return self.dev_db

    def show_vf(self, mt:str, port:int) -> list:
        pci = self.dev_db[mt][port]
        sysfs_device = f'/sys/bus/pci/devices/{pci}/'
        sysfs_vf_file = f'{sysfs_device}/mlx5_num_vfs'
        vf_num = int(self.rsh['cat'][sysfs_vf_file]())
        vf_pci = [None] * vf_num
        for i in range(0, vf_num):
            rcmd = self.rsh['ls']['-l', f'{sysfs_device}/virtfn{i}']
            vf_pci[i] = rcmd().strip('\n')[-PCI_ADDR_LEN:]
        return vf_pci

    def show_port_representors(self, mt:str, port:int) -> list:
        devlink = self.rsh['devlink']
        pci = self.dev_db[mt][port]
        params = f'dev eswitch show pci/{pci}'.split()
        if re.search('mode switchdev', devlink[params]()) is None:
            return []

        sysfs_device = f'/sys/bus/pci/devices/{pci}/'
        sysfs_vf_file = f'{sysfs_device}/mlx5_num_vfs'
        vf_num = int(self.rsh['cat'][sysfs_vf_file]())
        representors = [None] * vf_num
        for line in devlink['port']().split('\n'):
            if re.search(f'{pci}.*flavour pcivf', line) is not None:
                match = re.search('vfnum \d', line)
                vfnum = int(match.group(0).split()[-1])
                match = re.search('type eth netdev \w{1,}', line)
                representors[vfnum] = match.group(0).split()[-1]
        return representors

    def pci_to_netdev(self, pci:str) -> str:
        rdir=f'/sys/bus/pci/devices/{pci}/net'
        return self.rsh['ls'][rdir]().split('\n')[0]

    def link_up(self, netdev:str):
        ipcmd = self.rsh['ip']
        param = f'link set up dev {netdev}'.split()
        ipcmd[param]()

class DUTRemoteOps(RemoteOps):
    def __init__(self, rhost:str, **kwargs):
        RemoteOps.__init__(self, rhost, **kwargs)

    def config_hws(self, mt:str):
        _log = f'{self.rhost}: config HWS'
        utest_logger.info(_log)
        mst_dev = f'/dev/mst/{mt}_pciconf0'
        hws_status = self.rsh['mcra'][mst_dev, '0x1a3c.1:1']().split('\n')[0]
        if hws_status == '0x00000000':
            self.rsh['mcra'][mst_dev, '0x1a3c.1:1', '1']()
            self.rsh['/etc/init.d/openibd']['force-restart']()

    def config_vf(self, mt:str, port:int, num:int) -> int:
        pci = self.dev_db[mt][port]
        sysfs_device = f'/sys/bus/pci/devices/{pci}/'
        sysfs_vf_file = f'{sysfs_device}/mlx5_num_vfs'
        vf_num = int(self.rsh['cat'][sysfs_vf_file]())
        if vf_num < num:
            self.sysfs_config(sysfs_vf_file, 0)
            self.sysfs_config(sysfs_vf_file, str(num))
            vf_num = int(self.rsh['cat'][sysfs_vf_file]())
        return vf_num

    def config_fdb(self, mt:str, port:int):
        _log = f'{self.rhost}: config FDB on port{port}'
        utest_logger.info(_log)
        if len(self.show_port_representors(mt, port)): return

        pci = self.dev_db[mt][port]
        sysfs_bind = f'/sys/bus/pci/drivers/mlx5_core/bind'
        sysfs_unbind = f'/sys/bus/pci/drivers/mlx5_core/unbind'
        vf_pci_list = self.show_vf(mt, port)
        for vf_pci in vf_pci_list:
            utest_logger.debug(f'unbind: {vf_pci}')
            self.sysfs_config(sysfs_unbind, vf_pci)
        self.rsh['devlink']['dev', 'eswitch', 'set',
                            f'pci/{pci}', 'mode', 'switchdev']()
        for vf_pci in vf_pci_list:
            utest_logger.debug(f'bind: {vf_pci}')
            self.sysfs_config(sysfs_bind, vf_pci)

    def config_huge_pages(self, hp_num:int):
        rcmd = self.rsh['cat']['/sys/devices/system/node/online']
        out = rcmd().strip('\n')
        if re.search(',|-', out) is not None:
            last_node = rcmd().strip('\n').split('-')[1]
        else:
            last_node = out
        for i in range(0, int(last_node) + 1):
            sysfs_hp=f'/sys/devices/system/node/node{i}/' + \
                     'hugepages/hugepages-2048kB/nr_hugepages'
            self.sysfs_config(sysfs_hp, str(hp_num))























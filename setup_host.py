#! /usr/bin/env python3

import re
import logging
from remote_ops import *

utest_logger = logging.getLogger('unit-tests-logger')

def ssh_params(conf:dict) -> dict:
    params = {}
    keys = conf.keys()
    if 'username' in keys: params['username'] = conf['username']
    if 'password' in keys: params['password'] = conf['password']
    if 'keyfile' in keys: params['keyfile'] = conf['keyfile']
    return params

def select_mt_dev(mt_list:list, test_conf:dict) -> str:
    if 'hw' not in test_conf.keys() or test_conf['hw'] == 'any':
        mt_dev = mt_list[0]
    elif test_conf['hw'] in mt_list:
        mt_dev = test_conf['hw']
    else:
        mt_dev = None
    return mt_dev

NO_DUT_FW_RESET = 1

def setup_dut(test_conf:dict, dut_conf:dict, flags = 0) -> str:
    interfaces = {}

    dut = DUTRemoteOps(dut_conf['host'], **ssh_params(dut_conf)).connect()
    mt_db = dut.mst_status()
    mt_dev = select_mt_dev(list(mt_db.keys()), test_conf)
    _log = f'{dut.rhost}: mst_dev {mt_dev}'
    utest_logger.info(_log)

    if (flags & NO_DUT_FW_RESET) == NO_DUT_FW_RESET:
        _log = f'{dut.rhost}: FW reset suppressed'
        utest_logger.info(_log)
    else:
        dut.fw_reset(mt_dev)
        dut.connect()
    _log = f'{dut.rhost}: FW version ' + dut.fw_version()
    utest_logger.info(_log)

    if 'hws' in test_conf.keys() and test_conf['hws'] == True:
        dut.config_hws(mt_dev)
        dut.connect()

    if 'vf' in test_conf.keys():
        for port_id, vf_config in enumerate(test_conf['vf']):
            dut.config_vf(mt_dev, port_id, vf_config)

    for port_id, pf_conf in enumerate(test_conf['pf']):
        if pf_conf == 'fdb':
            dut.config_fdb(mt_dev, port_id)

    dut.config_huge_pages(1024)
    dut.disconnect()
    return mt_dev

def dut_interfaces(conf:dict, mt_dev:str) -> dict:
    interfaces = {}
    dut = DUTRemoteOps(conf['host'], **ssh_params(conf)).connect()
    mt_db = dut.mst_status()
    for pf_id, pf_pci in enumerate(mt_db[mt_dev]):
        pf_key = f'pf{pf_id}'
        interfaces[pf_key] = pf_pci

        for vf_id, vf_pci in enumerate(dut.show_vf(mt_dev, pf_id)):
            vf_key = f'{pf_key}vf{vf_id}'
            interfaces[vf_key] = vf_pci
    dut.disconnect()
    return interfaces

def host_interfaces(conf:dict, mt_dev:str) -> dict:
    interfaces = {}
    host = RemoteOps(conf['host'], **ssh_params(conf)).connect()
    mt_db = host.mst_status()
    for pf_id, pf_pci in enumerate(mt_db[mt_dev]):
        pf_key = f'pf{pf_id}'
        interfaces[pf_key] = host.pci_to_netdev(pf_pci)

        for vf_id, vf_pci in enumerate(host.show_vf(mt_dev, pf_id)):
            vf_key = f'{pf_key}vf{vf_id}'
            interfaces[vf_key] = host.pci_to_netdev(vf_pci)

        for rep_id, rep in enumerate(host.show_port_representors(mt_dev, pf_id)):
            rep_key = f'{pf_key}rf{rep_id}'
            interfaces[rep_key] = rep

    for netdev in interfaces.values():
        host.link_up(netdev)

    host.disconnect()
    return interfaces


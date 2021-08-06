#! /usr/bin/env python3

import logging, re
from time import sleep
from testpmd import TestPMD
from scapy.all import *


class UnitTestError(Exception):

    def __init__(self, test_id, pkt):
        self.test_id = test_id
        self.pkt = pkt


class UnitTestMatchError(UnitTestError):
    def __init__(self, test_id, pkt):
        UnitTestError.__init__(self, test_id, pkt)


class UnitTestNoCaptureError(UnitTestError):
    def __init__(self, test_id, pkt):
        UnitTestError.__init__(self, test_id, pkt)


class UTest:
    PROBE_OK = 0
    PROBE_NO_MATCH = 1
    PROBE_NO_CAPTURE = 2
    ASYNC_TMOUT = 0.1

    # lfilter = None

    def __init__(self, id:str, testpmd:TestPMD, config:dict):
        self.verbose = True
        self.id = id
        self.testpmd = testpmd
        self.ifin = config['ifin'] if 'ifin' in config.keys() else None
        self.ifout = config['ifout'] if 'ifout' in config.keys() else self.ifin

    def lfilter(self, pkt=None):
        return True

    def validate(self, sniffed=None):
        return self.PROBE_OK

    def send_recv(self, pkt, tmout):
        s = AsyncSniffer(lfilter=self.lfilter, iface=self.ifout, count=1, timeout=0.5)
        s.start()
        sleep(tmout)
        sendp(pkt, iface=self.ifin, count=1, verbose=False)
        logging.info('TG > ' + repr(pkt))
        self.testpmd.rdout()
        s.join()
        res = self.validate(s.results)
        if res == self.PROBE_NO_MATCH:
            raise UnitTestMatchError(self.id, pkt)
        elif res == self.PROBE_NO_CAPTURE:
            raise UnitTestNoCaptureError(self.id, pkt)

    def send(self, pkt, match=None):
        logging.info('TG > ' + repr(pkt))
        sendp(pkt, iface=self.ifin, count=1, verbose=False)
        sleep(.1)
        out = self.testpmd.rdout()
        if match is not None and [e for e in out if re.search(match, e)] == []:
            raise UnitTestMatchError(self.id, pkt)

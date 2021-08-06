#! /usr/bin/env python3

from rcmd import *
from re import sub

class TestPMDCmdError(RCmdError):
    def __init__(self, cmd):
        self.cmd = cmd

class TestPMD(RCmd):
    def __init__(self, config:dict):
        RCmd.__init__(self, config)
        self.execute([['show port summary all']])

    def flow_create(self, rules):
        for r in rules:
            self.execute([[sub(r'  {1,}', ' ', r),
                           '^Flow rule #[0-9]+ created']])

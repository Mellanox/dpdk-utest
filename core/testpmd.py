#! /usr/bin/env python3

from core.rcmd import RCmd


class TestPMD(RCmd):
    def __init__(self, config:dict):
        RCmd.__init__(self, config)
        self.execute([['show port summary all']])

    def flow_create(self, rules):
        for r in rules:
            self.execute([[r, '^Flow rule #[0-9]+ created']])

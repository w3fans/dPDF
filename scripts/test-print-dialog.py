#!/usr/bin/env python3
"""Print-dialog association must never accept an unrelated broker window."""
import importlib.util
from pathlib import Path
import unittest

spec = importlib.util.spec_from_file_location('printing', Path(__file__).with_name('check-windows-print.py'))
printing = importlib.util.module_from_spec(spec)
spec.loader.exec_module(printing)


class Ownership(unittest.TestCase):
    def check(self, start, pids, owners):
        return printing.owned_by_fixture(start, 42, lambda w: pids.get(w, 0), lambda w: owners.get(w, 0))

    def test_direct_and_broker_owned(self):
        self.assertTrue(self.check(1, {1:42}, {}))
        self.assertTrue(self.check(1, {1:500,2:42}, {1:2}))
        self.assertTrue(self.check(1, {1:500,2:500,3:42}, {1:2,2:3}))

    def test_unrelated_broker_zero_and_cycles(self):
        self.assertFalse(self.check(1, {1:500,2:88}, {1:2}))
        self.assertFalse(self.check(0, {}, {}))
        self.assertFalse(self.check(1, {1:500,2:500}, {1:2,2:1}))
        self.assertFalse(self.check(1, {}, {i:i+1 for i in range(1,100)}))


if __name__ == '__main__':
    unittest.main()

import sys
from pathlib import Path
import unittest
from unittest.mock import patch
sys.path.insert(0,str(Path(__file__).resolve().parent))
from drv_driven_handoff import startup_plan,verify_direct_startup


class StartupPlan(unittest.TestCase):
    def test_host_targets(self):
        self.assertEqual(startup_plan(True),(200,()))
        self.assertEqual(startup_plan(False),(50,tuple(range(60,201,10))))

    def test_command_and_monotonic_samples(self):
        text='> run200\nRUN: ramp=2000ms target=200Hz/6.2%\n'
        rows=[dict(stage=3,freq_chz=v) for v in (10005,15000,19990)]
        with patch('drv_capture.parse_dump',return_value=(rows,)):
            self.assertTrue(verify_direct_startup(text)['monotonic'])
            for bad in (text.replace('run200','run50'),text+'ehz210\n'):
                with self.assertRaises(ValueError):verify_direct_startup(bad)
        for values in ((10000,9999),(15000,14000),(20001,20002)):
            with patch('drv_capture.parse_dump',return_value=([dict(stage=3,freq_chz=v) for v in values],)):
                with self.assertRaises(ValueError):verify_direct_startup(text)

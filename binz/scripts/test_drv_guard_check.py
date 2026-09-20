import unittest
from pathlib import Path
from drv_guard_check import verify


class GuardCheckTests(unittest.TestCase):
    def setUp(self):
        self.text=(Path(__file__).resolve().parents[1]/'captures/guardinstall_guard01.txt').read_text()
    def test_retained_disabled_faults(self):
        self.assertEqual(verify(self.text),dict(timer_faults=3,poststop_refusals=18,outputs_off=True))
    def test_wrong_fault_timeout_or_poststop_accept_refuses(self):
        for old,new in [('reason=4','reason=8'),('host_backstop=0','host_backstop=1'),
                        ('POSTSTOP refused=6','POSTSTOP refused=5'),('mode=2','mode=1')]:
            with self.assertRaises(ValueError):verify(self.text.replace(old,new,1))
    def test_missing_finaloff_refuses(self):
        with self.assertRaises(ValueError):verify(self.text.split('FINALOFF')[0])

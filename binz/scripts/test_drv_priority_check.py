import unittest
from pathlib import Path
from drv_priority_check import verify
class PriorityTests(unittest.TestCase):
    def fixture(self):
        final=(Path(__file__).resolve().parents[1]/'captures/dutysplit_atomic_02.txt').read_text().split('FINALOFF\n')[1]
        row='PRIORITYCASE comp=64 com=128 sequence=123 expected=123 disabled=1\nPRIORITYCASE comp=64 com=64 sequence=132 expected=132 disabled=1\nPRIORITYCHECK restored=1 gate_authority=0\n'
        return row*3+'FINALOFF\n'+final
    def test_pass(self):self.assertEqual(verify(self.fixture())['trials'],3)
    def test_rejects(self):
        for old,new in [('sequence=132','sequence=123'),('restored=1','restored=0'),('disabled=1','disabled=0'),('comp=64','comp=0')]:
            with self.subTest(old=old),self.assertRaises(ValueError):verify(self.fixture().replace(old,new,1))

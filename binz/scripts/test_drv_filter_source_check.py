import unittest
from pathlib import Path
from drv_filter_source_check import verify

class SourceCheckTests(unittest.TestCase):
    def fixture(self):
        final=(Path(__file__).resolve().parents[1]/'captures/dutysplit_atomic_02.txt').read_text().split('FINALOFF\n')[1]
        return ('FILTERSOURCE bits=1023 visits=2 restored=1 disabled=1 gate_authority=0\n'*3)+'FINALOFF\n'+final
    def test_pass(self):self.assertEqual(verify(self.fixture())['trials'],3)
    def test_every_failed_check(self):
        for bit in range(10):
            with self.subTest(bit=bit),self.assertRaises(ValueError):
                verify(self.fixture().replace('bits=1023',f'bits={1023^(1<<bit)}',1))
    def test_malformed_or_unsafe(self):
        for old,new in [('visits=2','visits=1'),('restored=1','restored=0'),('disabled=1','disabled=0'),('gate_authority=0','gate_authority=1'),('FINALOFF','MISSING')]:
            with self.subTest(old=old),self.assertRaises(ValueError):verify(self.fixture().replace(old,new,1))
    def test_trial_count(self):
        with self.assertRaises(ValueError):verify(self.fixture().split('\n',1)[1])
    def test_restoration_configuration_not_live_output(self):
        row='FILTERRESTORE timer_diff=0 csr_saved=1073742465 csr_after=641\n'
        text=self.fixture().replace('FILTERSOURCE ',row+'FILTERSOURCE ')
        self.assertEqual(verify(text,require_detail=True)['trials'],3)
        for old,new in [('timer_diff=0','timer_diff=1'),('csr_after=641','csr_after=640')]:
            with self.subTest(old=old),self.assertRaises(ValueError):verify(text.replace(old,new,1))
        with self.assertRaises(ValueError):verify(self.fixture(),require_detail=True)

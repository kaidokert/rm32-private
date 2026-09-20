import unittest
from pathlib import Path
from drv_filter_stop import decode

class FilterStopTests(unittest.TestCase):
    def fixture(self):
        final=(Path(__file__).resolve().parents[1]/'captures/dutysplit_atomic_02.txt').read_text().split('FINALOFF\n')[1]
        return 'FILTERSTOP sr=0 dier=4 ccer=16 cr1=513 ccmr1=49408 tisel=256 cnt=2000 csr=1073742465 armed=1 enabled=1 nvic_enabled=1 nvic_pending=0 before_source_stop=1\nFINALOFF\n'+final
    def test_no_pending_but_enabled(self):
        v=decode(self.fixture());self.assertFalse(v['capture_pending'])
        self.assertTrue(v['peripheral_irq_enabled']);self.assertEqual(v['filter_code'],12)
    def test_pending_and_overcapture(self):
        v=decode(self.fixture().replace('sr=0','sr=1028'))
        self.assertTrue(v['capture_pending']);self.assertTrue(v['overcapture'])
    def test_invalid(self):
        for old,new in [('cnt=2000','cnt=65536'),('armed=1','armed=2'),('before_source_stop=1','before_source_stop=0'),('FINALOFF','MISSING')]:
            with self.subTest(old=old),self.assertRaises(ValueError):decode(self.fixture().replace(old,new))

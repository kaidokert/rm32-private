from pathlib import Path
import unittest
from drv_adc_cyclic_check import decode


class CyclicTests(unittest.TestCase):
    def test_hardware_normal_stall_normal(self):
        root=Path(__file__).resolve().parents[1]/'captures'
        for name in ['adc_cyclic_01','adc_cyclic_02']:
            h,rows=decode((root/(name+'.txt')).read_text())
            self.assertEqual(h['copy_max_us'],4);self.assertEqual(len(rows),32)
        h,rows=decode((root/'adc_cyclic_stall_01.txt').read_text())
        self.assertEqual(h['lease_fault'],2);self.assertEqual(rows,[])

    def test_unstopped_missing_or_stall_publication_refused(self):
        root=Path(__file__).resolve().parents[1]/'captures'
        normal=(root/'adc_cyclic_01.txt').read_text()
        stall=(root/'adc_cyclic_stall_01.txt').read_text()
        for text in [normal.replace('stopped=1','stopped=0'),normal.replace('CS85 ','MISSING ',1),
                     normal.replace('copy_max_us=4','copy_max_us=101'),
                     stall.replace('copied=0','copied=1'),stall.replace('lease_fault=2','lease_fault=3')]:
            with self.assertRaises(ValueError): decode(text)

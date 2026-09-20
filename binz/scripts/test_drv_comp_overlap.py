import unittest
from unittest.mock import patch
from drv_comp_overlap import decode


class OverlapTests(unittest.TestCase):
    def test_both_disabled_probe_failures_and_restorations_are_retained(self):
        from pathlib import Path
        from drv_cpu_check import verify
        from drv_capture import verify_off
        root=Path(__file__).resolve().parents[1]/'captures'
        for name in ['overlap_cpu01.txt','overlap_packed_cpu02.txt']:
            text=(root/name).read_text()
            verify_off(text[text.rfind('FINALOFF'):].encode())
            self.assertIn('max_us=11 fault=0',text)
            with self.assertRaises(ValueError):verify(text,union=True,comp_overlap=True)
        for name in ['overlap_restorecpu02.txt','overlap_packed_restorecpu01.txt']:
            self.assertEqual([r['max_pair_us'] for r in verify((root/name).read_text(),union=True)],[2,7,10])

    def test_strict_stop_only_evidence(self):
        line='COMPOVERLAP stop_in_comp=1 mask=66 guard_bit=2 dma_bit=64 extra_clock_reads=0 stop_context_only=1 preentry_delay_measured=0'
        with patch('drv_comp_overlap.cpu_decode',return_value=dict(valid=True,kind='irq_union')):
            r=decode(line,True)
            self.assertTrue(r['guard_overlap']);self.assertTrue(r['dma_overlap'])
            self.assertFalse(r['cause_identified']);self.assertIsNone(decode(''))
            for bad in ['',line+'\n'+line,line.replace('mask=66','mask=4'),
                        line.replace('stop_in_comp=1','stop_in_comp=0')]:
                with self.assertRaises(ValueError):decode(bad,True)
            self.assertFalse(decode(line.replace('mask=66','mask=0'))['guard_overlap'])


if __name__=='__main__':unittest.main()

import base64
import struct
import unittest
import zlib
from drv_adc_phase import decode


def fixture():
    text='ADCPHASE count=32 bins=32 period_ticks=6400 trigger_only=1 aperture_known=0 sector_known=0 latency_qualified=0 stopped=1\n'
    for i in range(32):
        raw=struct.pack('<3H',i,1,0)
        text+='AP85 '+base64.a85encode(raw+struct.pack('<I',zlib.crc32(raw))).decode()+'\n'
    return text


class PhaseTests(unittest.TestCase):
    def test_20k_requires_20k_not_24k_provenance(self):
        text=fixture().replace('period_ticks=6400','period_ticks=3200')
        marker='PWMROLES bemf_only=1 equal_ccr=1 initial_ug=1 per_com_ug=0 forced_startup_unchanged=1 carrier_hz=20000\n'
        self.assertEqual(decode(marker+text)['period_ticks'],3200)
        for bad in [text,marker.replace('20000','24006')+text,marker+fixture()]:
            with self.assertRaises(ValueError):decode(bad)
    def test_new_period_requires_matching_carrier_provenance(self):
        text=fixture().replace('period_ticks=6400','period_ticks=2666')
        marker='PWMROLES bemf_only=1 equal_ccr=1 initial_ug=1 per_com_ug=0 forced_startup_unchanged=1 carrier_hz=24006\n'
        self.assertEqual(decode(marker+text)['period_ticks'],2666)
        for bad in [text,marker.replace('24006','10000')+text,marker+text.replace('2666','2665'),marker+fixture()]:
            with self.assertRaises(ValueError):decode(bad)

    def test_real_powered_trigger_distribution(self):
        from pathlib import Path
        from drv_driven_handoff import verify
        text=(Path(__file__).resolve().parents[1]/'captures/adc_phase45_01.txt').read_text()
        result=decode(text)
        self.assertEqual(result['count'],4975)
        self.assertEqual((min(result['bins']),max(result['bins'])),(92,401))
        self.assertTrue(verify(text,1000)['powered_handoff_window_verified'])
        self.assertFalse(result['unbiased_current_proven'])

    def test_balanced_is_not_calibrated(self):
        result=decode(fixture());self.assertEqual(result['bins'],[1]*32)
        self.assertFalse(result['unbiased_current_proven']);self.assertIsNone(decode('legacy'))
    def test_bad_evidence(self):
        good=fixture()
        for text in [good+good,good.replace('count=32','count=33'),
                     good.replace('stopped=1','stopped=0'),good.replace('AP85 ','MISSING ',1)]:
            with self.assertRaises(ValueError):decode(text)

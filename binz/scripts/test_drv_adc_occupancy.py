import base64
import struct
import unittest
import zlib
from drv_adc_occupancy import decode


def fixture(bad=False):
    text='ADCLAUNCH bins=8 period_ticks=6400 limit=192 early_window=1 aperture_known=0 joint_sector_coverage=0\n'
    for phase in range(3):
        raw=struct.pack('<11H',phase,192,8 if bad else 0,*([24]*8))
        text+='AL85 '+base64.a85encode(raw+struct.pack('<I',zlib.crc32(raw))).decode()+'\n'
    return text


class OccupancyTests(unittest.TestCase):
    def test_driven_handoff_now_emits_existing_coverage(self):
        from pathlib import Path
        from drv_driven_handoff import verify
        text=(Path(__file__).resolve().parents[1]/'captures/current_coverage53_01.txt').read_text()
        data=decode(text)
        self.assertEqual(data['channels'][0]['bins'],[8,10,16,33,24,20,23,49])
        self.assertEqual([c['attempted'] for c in data['channels']],[192]*3)
        self.assertFalse(data['current_accuracy_qualified'])
        self.assertEqual(verify(text,1000)['powered']['adc_launch_records_verified'],1)

    def test_hardware_capture_and_campaign_validation(self):
        from pathlib import Path
        from drv_sustained_report import summarize
        text=(Path(__file__).resolve().parents[1]/'captures/launch40_01.txt').read_text()
        data=decode(text)
        self.assertEqual(data['channels'][2]['bins'],[6,22,26,27,24,28,27,24])
        self.assertEqual(summarize(text)['adc_launch_records_verified'],1)
        with self.assertRaises(ValueError): summarize(text.replace('AL85 ','MISSING ',1))

    def test_valid_is_not_current_certificate(self):
        result=decode(fixture())
        self.assertEqual(result['channels'][0]['bins'],[24]*8)
        self.assertFalse(result['current_accuracy_qualified'])

    def test_missing_duplicate_corrupt_or_impossible_counts(self):
        text=fixture()
        for bad in [fixture(True),text+text,text.replace('AL85','MISSING',1),
                    text.replace('limit=192','limit=200'),text[:-3]+'!!\n']:
            with self.assertRaises(ValueError): decode(bad)

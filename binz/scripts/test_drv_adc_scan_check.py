import base64
from pathlib import Path
import struct
import unittest
import zlib
from drv_adc_scan_check import decode


def fixture():
    text='ADCSCAN result=0 scans=32 words=160 elapsed_us=3290 remaining=0 stopped=1 order=0,1,4,6,13 trigger_us=101 gate_authority=0\n'
    for i in range(32):
        raw=struct.pack('<6H',i,100,200,300,1200,1500)
        text+='AS85 '+base64.a85encode(raw+struct.pack('<I',zlib.crc32(raw))).decode()+'\n'
    safe=(Path(__file__).resolve().parents[1]/'captures/adc_trigger_01.txt').read_text()
    return text+safe[safe.rfind('OUT:'):]


class ScanTests(unittest.TestCase):
    def test_real_stopped_scan(self):
        text=(Path(__file__).resolve().parents[1]/'captures/adc_scan_01.txt').read_text()
        result=decode(text)
        self.assertEqual(result['elapsed_us'],3288)
        self.assertEqual(len(result['scans']),32)
        self.assertTrue(all(1503<=r['vref']<=1510 for r in result['scans']))

    def test_ascending_scan_decodes_to_logical_abc(self):
        result=decode(fixture())
        self.assertEqual(result['scans'][0],dict(a=300,b=200,c=100,bus=1200,vref=1500))
        self.assertFalse(result['measurement_accuracy_qualified'])

    def test_incomplete_or_unstopped_capture_refused(self):
        text=fixture()
        for bad in [text.replace('remaining=0','remaining=5'),text.replace('stopped=1','stopped=0'),
                    text.replace('AS85 ','MISSING ',1),text.replace('OUT:','MISSING:')]:
            with self.assertRaises((ValueError,RuntimeError)): decode(bad)

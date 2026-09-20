import base64
import struct
import sys
import unittest
import zlib
from pathlib import Path
sys.path.insert(0,str(Path(__file__).resolve().parent))
from drv_rate_snapshot import HEADER, decode

def record(count, average=1666):
    raw=struct.pack('<8H',count,average,0,1,1,1,0,2)
    return 'RG85 '+base64.a85encode(raw+struct.pack('<I',zlib.crc32(raw))).decode()+'\n'

class Snapshot(unittest.TestCase):
    def test_gate_boundary_and_no_history_claim(self):
        for count, opened in [(832,False),(833,False),(834,True)]:
            result=decode(HEADER+'\n'+record(count))
            self.assertEqual(result['gate_open'],opened)
            self.assertFalse(result['historical_cause_proven'])
    def test_absent_and_invalid(self):
        self.assertIsNone(decode(HEADER+'\n'))
        for text in [record(834),HEADER+'\n'+record(834)*2,
                     HEADER+'\n'+record(834,1000),HEADER+'\nRG85 !!!!!\n']:
            with self.assertRaises(ValueError): decode(text)

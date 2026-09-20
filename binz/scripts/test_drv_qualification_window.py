import base64
import struct
import unittest
import zlib
from drv_qualification_window import decode

def record(label,values):
    raw=struct.pack('<'+'H'*len(values),*values)
    return label+' '+base64.a85encode(raw+struct.pack('<I',zlib.crc32(raw))).decode()

def sample(row=None,pending=None):
    return '\n'.join(['QUALWINDOW n=1 omitted=0 invalid=0 frozen=1 capture=1 dispatch_body=1 edge_time=0',
        record('QW85',row or [1,0,3,1,0,10,2,35,0]),record('QP85',pending or [0,0,0,0])])

class QualificationWindowTests(unittest.TestCase):
    def test_valid_accept_and_stopped_refusal(self):
        r=decode(sample(),True);self.assertEqual(r['rows'][0]['calls'],10)
        self.assertFalse(r['physical_edge_time'])
        r=decode(sample([0,0,3,0,1,4,1,40,0]),True)
        self.assertTrue(r['rows'][0]['stopped']);self.assertFalse(r['rows'][0]['accepted'])
    def test_missing_quiet_unused_and_orphans(self):
        self.assertIsNone(decode(''))
        quiet=sample().splitlines()[0].replace('capture=1','capture=0')
        self.assertFalse(decode(quiet)['captured'])
        for text in ['',quiet,'\n'.join(sample().splitlines()[1:]),
                     sample().replace('n=1','n=0'),sample()+'\n'+sample()]:
            with self.assertRaises(ValueError):decode(text,True)
    def test_corrupt_crc_flags_counters_and_ordinals(self):
        for text in [sample().replace('invalid=0','invalid=1'),
                     sample().replace('frozen=1','frozen=0'),
                     sample().replace('QW85 ','QW85 !',1),
                     sample([2,0,3,1,0,10,2,35,0]),
                     sample([1,0,7,1,0,10,2,35,0]),
                     sample([1,0,3,1,0,1,2,35,0]),
                     sample([0,0,3,0,1,4,1,40,0],[1,0,1,0])]:
            with self.assertRaises(ValueError):decode(text,True)
    def test_rollover_accounting(self):
        head='QUALWINDOW n=16 omitted=20 invalid=0 frozen=1 capture=1 dispatch_body=1 edge_time=0'
        text='\n'.join([head]+[record('QW85',[21+i,0,i%6+1,1,0,10,0,7,0]) for i in range(16)]+[record('QP85',[0,0,0,0])])
        self.assertEqual(decode(text,True)['rows'][-1]['ordinal'],36)

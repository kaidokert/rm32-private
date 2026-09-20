import base64
import struct
import unittest
import zlib
from drv_observation import decode,FIELDS,agreement

class ObservationDecode(unittest.TestCase):
    def frame(self):
        raw=struct.pack('<16H',500,0,500,1,2,1600,20,100,90,2048,2048,2048,1200,1500,0,6100)
        raw+=struct.pack('<I',zlib.crc32(raw))
        return 'OBS n=1 wire=a85-v1 fields='+','.join(FIELDS)+'\nB85 '+base64.a85encode(raw).decode()+'\nOBS END'
    def test_roundtrip(self):
        row=decode(self.frame())[0]
        self.assertEqual(row['us'],500)
        self.assertEqual(row['c_minus_neutral'],10)
    def test_wire_versions(self):
        for version in (1,2,3,4,5,6,7,8):
            row=decode(self.frame().replace('a85-v1',f'a85-v{version}'))[0]
            self.assertEqual(row['wire_version'],version)
        with self.assertRaises(ValueError):
            decode(self.frame().replace('a85-v1','a85-v9'))
    def test_agreement(self):
        row=decode(self.frame().replace('a85-v1','a85-v4'))[0]
        row.update(c_minus_neutral=20,on=1,late_on=0)
        self.assertEqual(agreement([row]),dict(n=1,early_match=0,late_match=1,early_late_changes=1))
        row['reject']=2
        self.assertEqual(agreement([row])['n'],0)
    def test_v8_preserves_raw_and_changes_only_reference_level(self):
        old=decode(self.frame().replace('a85-v1','a85-v7'))[0]
        new=decode(self.frame().replace('a85-v1','a85-v8'))[0]
        self.assertEqual(old['late_on'],new['late_on'])
        self.assertEqual(old['flags'],new['flags'])
        self.assertEqual(old['reference_late_on'],1-new['reference_late_on'])
    def test_bad_frames(self):
        frame=self.frame()
        for bad in [frame.replace('n=1','n=2'),frame.replace('OBS END',''),frame.replace('B85 ','B85 !!!!!')]:
            with self.assertRaises(ValueError):decode(bad)
    def test_absent_analog(self):
        raw=struct.pack('<16H',500,0,500,1,2,60,28,65535,65535,2048,2048,2048,1200,1500,0,350)
        raw+=struct.pack('<I',zlib.crc32(raw))
        frame='OBS n=1 wire=a85-v5 fields='+','.join(FIELDS)+'\nB85 '+base64.a85encode(raw).decode()+'\nOBS END'
        row=decode(frame)[0]
        self.assertFalse(row['analog_present'])
        self.assertIsNone(row['c_minus_neutral'])
        self.assertEqual(agreement([row])['n'],0)
    def test_pwm_slots(self):
        raw=struct.pack('<16H',500,0,500,1,2,60,28,1,4,2048,2048,2048,1200,1500,0,350)
        raw+=struct.pack('<I',zlib.crc32(raw))
        frame='OBS n=1 wire=a85-v6 fields='+','.join(FIELDS)+'\nB85 '+base64.a85encode(raw).decode()+'\nOBS END'
        row=decode(frame)[0]
        self.assertEqual(row['pwm_128'],1)
        self.assertEqual(row['pwm_224'],0)
        self.assertIsNone(row['pwm_320'])
        self.assertIsNone(row['late_on'])
        self.assertIsNone(row['c_minus_neutral'])
    def test_live_decision_fields(self):
        raw=struct.pack('<16H',500,0,500,1,2,60,508,4,10,2048,2048,2048,1200,1500,64,350)
        raw+=struct.pack('<I',zlib.crc32(raw))
        frame='OBS n=1 wire=a85-v7 fields='+','.join(FIELDS)+'\nB85 '+base64.a85encode(raw).decode()+'\nOBS END'
        row=decode(frame)[0]
        self.assertEqual([row[k] for k in ('decision_expected','decision_armed','decision_event','decision_latched','decision_reason')],[1,1,1,1,4])
        self.assertIsNone(row['c_minus_neutral'])

if __name__=='__main__':unittest.main()

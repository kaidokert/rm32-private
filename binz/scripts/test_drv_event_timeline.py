import base64
import struct
import unittest
import zlib
from pathlib import Path
from drv_event_timeline import decode


def record(index,count=100,minimum=700,maximum=800):
    raw=struct.pack('<5H',index,count&65535,count>>16,minimum,maximum)
    return 'ET85 '+base64.a85encode(raw+struct.pack('<I',zlib.crc32(raw))).decode()


def fixture():
    return '\n'.join(['TIMELINE label=ET85 bins=6 window_us=30000000 bin_us=5000000 events=600 overrun_events=0 accepted_only=1',
                      'TIMELINEREFUSED n=0','ACCEPTQUALITY events=600 last_us=29999000 end_us=30000000']+
                     [record(i) for i in range(6)])


class TimelineTests(unittest.TestCase):
    def test_real_recovery_preserves_two_independent_timelines(self):
        text=(Path(__file__).resolve().parents[1]/'captures/reentry_timeline_arm_01.txt').read_text()
        first=decode(text,'FT85');second=decode(text,'ET85')
        self.assertEqual(first['header']['events'],2839)
        self.assertEqual(second['header']['events'],11379)
        self.assertEqual(first['bins'][2]['observed_us'],0)
        self.assertEqual([b['events'] for b in second['bins']],[1895,1895,1897,1897,1898,1897])
        self.assertLess(first['end_us'],second['end_us'])

    def test_complete_and_large_counts(self):
        t=decode(fixture());self.assertEqual(t['bins'][5]['observed_us'],5000000)
        text=fixture().replace(record(0),record(0,70000)).replace('events=600','events=70500')
        self.assertEqual(decode(text)['bins'][0]['events'],70000)

    def test_bad_crc_order_count_and_metadata_refused(self):
        text=fixture()
        changes=[text.replace(record(1),record(2)),text.replace(record(1),''),
                 text.replace(record(1),record(1,99)),text.replace('bin_us=5000000','bin_us=1'),
                 text.replace('last_us=29999000','last_us=30000001'),text.replace('n=0','n=1'),
                 text+'\n'+text.splitlines()[0],text.replace(record(1),record(1)[:-1]+'!')]
        for changed in changes:
            with self.assertRaises(ValueError): decode(changed)

    def test_partial_observation_does_not_create_tail_activity(self):
        text=fixture().replace('end_us=30000000','end_us=7000000').replace('last_us=29999000','last_us=6999000')
        with self.assertRaises(ValueError): decode(text)
        for i in range(2,6): text=text.replace(record(i),record(i,0,65535,0))
        text=text.replace('events=600','events=200')
        t=decode(text)
        self.assertEqual(t['bins'][1]['observed_us'],2000000)
        self.assertEqual(t['bins'][2]['observed_us'],0)

    def test_first_segment_uses_its_own_totals_and_clock(self):
        text=fixture().replace('ET85','FT85').replace('ACCEPTQUALITY','FIRSTSEG')
        self.assertEqual(decode(text,'FT85')['header']['events'],600)

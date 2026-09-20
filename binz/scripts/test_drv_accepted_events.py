import base64
import struct
import unittest
import zlib
import re
from pathlib import Path
from drv_accepted_events import decode, decode_windows


class AcceptedTests(unittest.TestCase):
    def windows(self, total):
        def wire(n, tag):
            us=800*(n+1)
            raw=struct.pack('<4H',us&65535,us>>16,n%6+1,1600)
            raw+=struct.pack('<I',zlib.crc32(raw))
            return tag+' '+base64.a85encode(raw).decode()
        prefix=min(total,32); tail=min(max(total-32,0),32)
        return '\n'.join([
            f'ACCEPTLOG n={prefix} drop={total-prefix} fields=test',
            *(wire(i,'A85') for i in range(prefix)),
            f'ACCEPTTAIL n={tail} skipped={max(total-64,0)} total={total} fields=test',
            *(wire(i,'T85') for i in range(total-tail,total))])

    def test_explicit_windows_preserve_middle_gap(self):
        for total in (0,1,32,33,64,65,144000):
            with self.subTest(total=total):
                result=decode_windows(self.windows(total))
                self.assertEqual(result['total'],total)
                self.assertEqual(result['skipped'],max(total-64,0))
                self.assertEqual(len(result['prefix'])+len(result['tail'])+result['skipped'],total)
                if total>32:
                    with self.assertRaises(ValueError): decode(self.windows(total))
                    self.assertEqual(result['tail'][-1]['us'],total*800)

    def test_windows_reject_corruption_and_false_accounting(self):
        text=self.windows(100)
        for bad in (text.replace('skipped=36','skipped=35'),
                    text.replace('total=100','total=99'),
                    text.replace('drop=68','drop=0'),
                    text.rsplit('\n',1)[0], text+'\nT85 !!!!!'):
            with self.assertRaises(ValueError): decode_windows(bad)

    def test_legacy_windows_still_reject_missing_data(self):
        self.assertEqual(decode_windows(self.text())['prefix'],decode(self.text()))
        with self.assertRaises(ValueError): decode_windows(self.text().replace('drop=0','drop=1'))

    def test_hardware_whole_stream_moments_match_retained_short_windows(self):
        root=Path(__file__).resolve().parents[1]/'captures'
        for tag in ('sustain55_200_01','sustain50_200_01','sustain45_200_10s_03'):
            with self.subTest(tag=tag):
                text=(root/f'{tag}.txt').read_text(); windows=decode_windows(text)
                self.assertEqual(windows['skipped'],0)
                events=windows['prefix']+windows['tail']
                gaps=[b['us']-a['us'] for a,b in zip(events,events[1:])]
                cycles=[b['us']-a['us'] for a,b in zip(events,events[6:])]
                for kind,values in [('gap',gaps),('cycle',cycles)]:
                    fields=re.search(rf'ACCEPTMOMENTS kind={kind} n=(\d+) min_us=(\d+) max_us=(\d+) sum_us=(\d+) squares_us=(\d+) excluded=(\d+)',text)
                    self.assertEqual(tuple(map(int,fields.groups())),
                        (len(values),min(values,default=0),max(values,default=0),sum(values),sum(v*v for v in values),0))

    def test_ten_second_runs_keep_full_width_tail_and_explicit_gaps(self):
        root=Path(__file__).resolve().parents[1]/'captures'
        for tag,total in [('sustain45_200_10s_01',14191),('sustain45_200_10s_02',14214)]:
            text=(root/f'{tag}.txt').read_text(); windows=decode_windows(text)
            self.assertEqual(windows['total'],total)
            self.assertEqual(windows['skipped'],total-64)
            self.assertGreater(windows['tail'][0]['us'],9_970_000)
            self.assertIn('COASTREF stop=1 max_us=10000000',text)
            self.assertIn('desync=0 polling=0 running=1',text)
            with self.assertRaises(ValueError): decode(text)

    def test_measured_seed_coast_progress_has_no_initial_stale_window(self):
        for tag in (f'coasttrack_200_{n:02}' for n in range(1,9)):
            with self.subTest(tag=tag):
                text=(Path(__file__).resolve().parents[1]/f'captures/{tag}.txt').read_text()
                events=decode(text)
                self.assertEqual(len(events),22)
                seed=int(re.search(r'FLY result=1 step=(\d+)',text).group(1))
                self.assertEqual([e['step'] for e in events],[(i+seed)%6+1 for i in range(22)])
                stamps=[0]+[e['us'] for e in events]
                end=int(re.search(r'observed_end_us=(\d+)',text).group(1))
                self.assertLessEqual(end-stamps[-1],1000)
                self.assertLessEqual(stamps[1],1000)
                self.assertTrue(all(666<=b-a<=1000 for a,b in zip(stamps[1:],stamps[2:])))
                self.assertLessEqual(int(re.search(r'energized_us=(\d+)',text).group(1)),5_000_000)
                self.assertGreaterEqual(end,20_000)
                safe=(Path(__file__).resolve().parents[1]/f'captures/{tag}_safe.txt').read_text()
                self.assertRegex(safe,r'OUT: ah=0 bh=0 ch=0 al=0 bl=0 cl=0.*en=0.*moe=0 ccrA=0 ccrB=0 ccrC=0')
                self.assertRegex(safe,r'IN: ah=0 bh=0 ch=0 al=0 bl=0 cl=0.*en=0 nflt=1')
                self.assertIn('ACCEPTTIMING fault=0',text)
                self.assertIn('COASTREF stop=1',text)
                self.assertIn('desync=0 polling=0 running=1',text)
                self.assertIn('com=23 gate_authority=0',text)
                self.assertIn('CORESEED assumed=0 source=measured_flying',text)
                self.assertIn('bootstrap_com=1 synthetic_accept=0',text)
                self.assertLessEqual(int(re.search(r'arm_us=(\d+)',text).group(1)),16)

    def test_nvic_fix_repeats_post_acquisition_progress_not_acquisition_pass(self):
        cases=[('coastref_nvic_200_01',1,21,2480,791,954),
               ('coastref_nvic_200_02',1,22,1836,791,942),
               ('coastref_nvic_seed4_200_01',4,21,2439,793,955)]
        for tag,seed,count,first,low,high in cases:
            with self.subTest(tag=tag):
                text=(Path(__file__).resolve().parents[1]/f'captures/{tag}.txt').read_text()
                events=decode(text)
                self.assertEqual(len(events),count)
                self.assertEqual(events[0]['us'],first)
                self.assertEqual([e['step'] for e in events],[(seed-1+i)%6+1 for i in range(count)])
                gaps=[b['us']-a['us'] for a,b in zip(events,events[1:])]
                self.assertEqual((min(gaps),max(gaps)),(low,high))
                self.assertIn('software_masked=0 hardware_masked=0 not_pending=0',text)
                self.assertIn('COASTREF stop=1',text)
                self.assertIn('ACCEPTTIMING fault=1',text)
                self.assertIn('gate_authority=0',text)

    def test_retained_continuous_coast(self):
        text=(Path(__file__).resolve().parents[1]/'captures/coastref1_200_01.txt').read_text()
        events=decode(text)
        self.assertEqual(len(events),21)
        self.assertEqual([e['step'] for e in events],[i%6+1 for i in range(21)])
        gaps=[b['us']-a['us'] for a,b in zip(events,events[1:])]
        self.assertEqual(gaps[0],1335)
        self.assertEqual((min(gaps[1:]),max(gaps[1:])),(775,959))
        self.assertEqual(20008-events[-1]['us'],2121)
        self.assertIn('gates_disabled=1 sense_mux_only=1',text)
        self.assertIn('continuous=1 lock_proven=0',text)

    def test_retained_live_prefix(self):
        text=(Path(__file__).resolve().parents[1]/'captures/bemf_acceptlog_200_01.txt').read_text()
        events=decode(text)
        self.assertEqual(events,[dict(us=253,step=1,reference_interval_ticks=913)])
        self.assertIn('intervals=0 observed_end_us=493',text)
        self.assertIn('report_only=1 continuous=0 lock_proven=0',text)

    def text(self, step=1):
        raw=struct.pack('<4H',217,0,step,855)
        raw+=struct.pack('<I',zlib.crc32(raw))
        return 'ACCEPTLOG n=1 drop=0 fields=test\nA85 '+base64.a85encode(raw).decode()

    def test_roundtrip(self):
        self.assertEqual(decode(self.text()),[dict(us=217,step=1,reference_interval_ticks=855)])

    def test_incomplete_or_bad_sector_rejected(self):
        for text in [self.text().replace('drop=0','drop=1'),
                     self.text().replace('n=1','n=2'),self.text(0),self.text(7)]:
            with self.assertRaises(ValueError): decode(text)

    def test_crc_rejected(self):
        text=self.text()
        with self.assertRaises(ValueError): decode(text[:-1]+('!' if text[-1]!='!' else '#'))


if __name__ == '__main__': unittest.main()

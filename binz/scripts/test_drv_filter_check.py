import unittest
from pathlib import Path
from drv_filter_check import verify

class FilterCheckTests(unittest.TestCase):
    def test_mirror_does_not_consume_authority_event(self):
        extra=''.join(f'FILTERACK index={i} before={6 if cap else 0} after_mirror={4 if cap else 0} after_source=0 source_cc2=1 mirror_cc1=1\n' for i,cap in enumerate([1,1,0,1]))
        text=self.fixture().replace('FILTERCHECK ',extra+'FILTERCHECK ')
        self.assertTrue(all(r['mirror_ack_isolated'] for r in verify(text,require_ack=True)))
        for old,new in [('after_mirror=4','after_mirror=0'),('after_mirror=4','after_mirror=6'),('after_source=0','after_source=4')]:
            with self.subTest(new=new),self.assertRaises(ValueError):verify(text.replace(old,new,1),require_ack=True)
        with self.assertRaises(ValueError):verify(self.fixture(),require_ack=True)
    def test_one_us_candidate_passes_short_pulse(self):
        extra=''.join(f'FILTERPART code={c} requested_us={w} measured_us={w+1} levels=2 captured={cap} overcapture=0 ccr=123 en_low=1\n' for c,w,cap in [(12,2,0),(12,40,1),(5,2,1),(5,40,1)])
        text=self.fixture().replace('FILTERCHECK ',extra+'FILTERCHECK ')
        self.assertEqual(len(verify(text)),8)
        with self.assertRaises(ValueError):verify(text.replace('code=5 requested_us=2 measured_us=3 levels=2 captured=1','code=5 requested_us=2 measured_us=3 levels=2 captured=0'))
    def fixture(self):
        final=(Path(__file__).resolve().parents[1]/'captures/dutysplit_atomic_02.txt').read_text().split('FINALOFF\n')[1]
        rows=[f'FILTERPART code={c} requested_us={w} measured_us={w+1} levels=2 captured={cap} overcapture=0 ccr=123 en_low=1'
              for c,w,cap in [(0,2,1),(0,40,1),(15,2,0),(15,40,1)]]
        return '\n'.join(rows)+'\nFILTERCHECK restored=1 disabled=1 irq_enabled=0 gate_authority=0\nFINALOFF\n'+final
    def test_expected_sequence(self):self.assertEqual(len(verify(self.fixture())),4)
    def test_tim3_independence(self):
        extra='\n'.join(f'FILTERT3 index={i} captured=1 over=0 ccr=0 before=200 period=201 tick_us=1 filter=0' for i in range(4))+'\n'
        text=self.fixture().replace('FILTERCHECK ',extra+'FILTERCHECK ')
        self.assertEqual(verify(text,require_t3=True)[0]['raw3_delay_us'],1)
        for old,new in [('captured=1 over=0','captured=0 over=0'),('ccr=0 before=200','ccr=8 before=200'),('period=201','period=200')]:
            with self.subTest(old=old),self.assertRaises(ValueError):verify(text.replace(old,new),require_t3=True)
        with self.assertRaises(ValueError):verify(self.fixture(),require_t3=True)
    def test_pair_characterization(self):
        pairs='\n'.join(f'FILTERPAIR index={i} indirect_captured={1 if i!=2 else 0} indirect_over=0 indirect_ccr=123 ic1f=0 cc1s=2' for i in range(4))+'\n'
        text=self.fixture().replace('FILTERCHECK ',pairs+'FILTERCHECK ')
        self.assertEqual(verify(text)[2]['indirect_captured'],0)
        for old,new in [('indirect_over=0','indirect_over=1'),('index=3','index=4'),('ic1f=0','ic1f=12')]:
            with self.subTest(old=old),self.assertRaises(ValueError):verify(text.replace(old,new,1))
    def test_short_filter_rows(self):
        extra='FILTERPART code=12 requested_us=2 measured_us=3 levels=2 captured=0 overcapture=0 ccr=123 en_low=1\nFILTERPART code=12 requested_us=40 measured_us=41 levels=2 captured=1 overcapture=0 ccr=123 en_low=1\n'
        text=self.fixture().replace('FILTERCHECK ',extra+'FILTERCHECK ')
        self.assertEqual(len(verify(text)),6)
        with self.assertRaises(ValueError):verify(text.replace('code=12 requested_us=2 measured_us=3','code=12 requested_us=2 measured_us=7'))
    def test_timing(self):
        text=self.fixture()
        times=[]
        for i,(before,elapsed) in enumerate([(120,3),(120,41),(120,3),(93,41)]):
            times.append(f'FILTERTIME index={i} before={before} after={before+2*elapsed} elapsed_us={elapsed} post=0')
        text=text.replace('FILTERCHECK ', '\n'.join(times)+'\nFILTERCHECK ')
        self.assertEqual(verify(text,True)[3]['latency_half_us'],30)
        for old,new in [('after=126','after=140'),('post=0','post=1'),('before=93','before=60')]:
            with self.subTest(old=old),self.assertRaises(ValueError):verify(text.replace(old,new,1),True)
        with self.assertRaises(ValueError):verify(self.fixture(),True)
    def test_bad_level_width_overcapture_restore_and_sequence(self):
        text=self.fixture()
        for old,new in [('levels=2','levels=3'),('measured_us=3','measured_us=15'),
                        ('overcapture=0','overcapture=1'),('restored=1','restored=0'),
                        ('code=15 requested_us=2','code=0 requested_us=2')]:
            with self.subTest(old=old),self.assertRaises(ValueError):verify(text.replace(old,new,1))

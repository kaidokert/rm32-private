import unittest
from pathlib import Path
from drv_coast_periods import periods,period_spans,coast_bounds,analyze


class CoastPeriodTests(unittest.TestCase):
    def test_fast_coast_after_current_stop_is_not_completed_window(self):
        from drv_fast_coast import verify
        from drv_sustained_report import summarize
        text=(Path(__file__).resolve().parents[1]/'captures/fastcoast_628_ramp220_30s.txt').read_text()
        self.assertEqual(summarize(text)['powered_reason'],'5')
        self.assertEqual(verify(text,True)['early_scan_max_us'],82)
        with self.assertRaises(ValueError):analyze(text,30)
        phases=coast_bounds(text,30)
        self.assertEqual([r['phase'] for r in phases],list('ABC'))
        for phase in phases:
            self.assertEqual(len(phase['spans']),2)
            for span in phase['spans']:
                self.assertEqual(span['cycles'],3)
                self.assertGreater(span['average_frequency_lower_hz'],900)
                self.assertLess(span['average_frequency_upper_hz'],1150)

    def test_long_span_counts_full_cycles_without_rate_prior(self):
        samples=[(i*125,i*125+10,(i//4)%2) for i in range(32)]
        spans=period_spans(samples)
        self.assertTrue(spans)
        for span in spans:
            self.assertGreaterEqual(span['cycles'],2)
            self.assertLessEqual(span['average_frequency_lower_hz'],1000)
            self.assertGreaterEqual(span['average_frequency_upper_hz'],1000)
            self.assertGreater(span['average_frequency_lower_hz'],500)
        self.assertEqual(period_spans([(0,10,False),(125,135,False)]),[])
        with self.assertRaises(ValueError):period_spans([(0,20,False),(10,30,True)])

    def test_full_periods_not_half_periods(self):
        samples=[(i*500,i*500+10,(i//4)%2) for i in range(32)]
        result=periods(samples)
        self.assertTrue(result)
        self.assertTrue(all(p['frequency_lower_hz']<=250<=p['frequency_upper_hz'] for p in result))
        self.assertTrue(all(p['frequency_lower_hz']>125 for p in result))

    def test_bad_order_and_no_motion(self):
        self.assertEqual(periods([(0,10,False),(500,510,False)]),[])
        with self.assertRaises(ValueError): periods([(0,20,False),(10,30,True)])

    def test_real_coast_and_missing_offsets(self):
        text=(Path(__file__).resolve().parents[1]/'captures/launch45_reentry30_01.txt').read_text()
        result=analyze(text,100)
        self.assertFalse(result['lock_proven'])
        self.assertEqual(len(result['phases']),3)
        with self.assertRaises(ValueError): analyze(text.replace('COASTCOMP row=0','MISSING row=0'),100)

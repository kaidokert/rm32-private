import unittest
from unittest.mock import patch
from drv_driven_handoff import verify_quiet_irq_stamp


class QuietStampTests(unittest.TestCase):
    def test_recovery_pass_and_late_failure_remain_distinct(self):
        from pathlib import Path
        from drv_driven_handoff import verify
        from drv_cycle_fault import context
        root=Path(__file__).resolve().parents[1]/'captures'
        good=(root/'quietstamp_start61_reentry68_30s_01.txt').read_text()
        verify_quiet_irq_stamp(good)
        self.assertTrue(verify(good,30000,dropout=True,reentry=True)['powered_recovery_verified'])
        bad=(root/'quietstamp_start61_reentry69_30s_01.txt').read_text()
        verify_quiet_irq_stamp(bad)
        self.assertEqual(context(bad)['guard']['delta_us'],3017)
        with self.assertRaises(ValueError):verify(bad,30000,dropout=True,reentry=True)

    def test_retained_hold_provenance_and_completion(self):
        from pathlib import Path
        from drv_driven_handoff import verify
        text=(Path(__file__).resolve().parents[1]/'captures/quietstamp_start61_hold68_01.txt').read_text()
        verify_quiet_irq_stamp(text)
        self.assertTrue(verify(text,10000)['powered_handoff_window_verified'])

    @patch('drv_driven_handoff.verify_core_trace')
    def test_strict_provenance(self,trace):
        line='IRQSTAMP omitted=1 report_only=1 accepted_clock_unchanged=1 safety_clocks_unchanged=1'
        verify_quiet_irq_stamp(line);trace.assert_called_with(line,0)
        for bad in ['',line+'\n'+line,line.replace('omitted=1','omitted=0'),
                    line.replace('safety_clocks_unchanged=1','safety_clocks_unchanged=0')]:
            with self.assertRaises(ValueError):verify_quiet_irq_stamp(bad)

import unittest
from pathlib import Path
from drv_driven_handoff import verify_reentry_clear


class ReentryClearTests(unittest.TestCase):
    def test_retained_recovery_has_marker_and_measured_timing(self):
        from drv_timing_report import report
        text=(Path(__file__).resolve().parents[1]/'captures/clearonce_reentry68_30s_01.txt').read_text()
        verify_reentry_clear(text,required=True)
        result=report(text,reentry=True)
        self.assertEqual(result['seed_stage_timing']['arm_age_us'],85)
        self.assertEqual(result['segment_arm_spare_above_floor_us'],15.5)
        self.assertEqual(result['original_deadline_spare_us'],215)

    def test_strict_optional_provenance(self):
        line='REENTRYCLEAR acquisition_clear=1 duplicate_omitted=1 live_checks=1 initial_unchanged=1'
        verify_reentry_clear('')
        verify_reentry_clear(line,required=True)
        for bad in ['',line+'\n'+line,line+' extra=1',
                    line.replace('live_checks=1','live_checks=0'),
                    line.replace('initial_unchanged=1','initial_unchanged=0')]:
            with self.assertRaises(ValueError):verify_reentry_clear(bad,required=True)


if __name__=='__main__':unittest.main()

import unittest
from pathlib import Path
from drv_driven_handoff import verify_carrier_math


class CarrierMathTests(unittest.TestCase):
    def test_retained_recovery_and_timing(self):
        from drv_timing_report import report
        text=(Path(__file__).resolve().parents[1]/'captures/carriermath_reentry68_30s_01.txt').read_text()
        verify_carrier_math(text,required=True)
        result=report(text,reentry=True)
        self.assertEqual(result['commit_bracket_max_us'],41)
        self.assertEqual(result['com_bracket_max_us'],67)
        self.assertEqual(result['seed_stage_timing']['arm_age_us'],85)
        self.assertEqual(result['original_deadline_spare_us'],191)

    def test_strict_optional_provenance(self):
        line='CARRIERMATH compare_at_prepare=1 steady_divide=0 exact_compare=1 duty_change_refused=1'
        verify_carrier_math('')
        verify_carrier_math(line,required=True)
        for bad in ['',line+'\n'+line,line+' extra=1',
                    line.replace('steady_divide=0','steady_divide=1'),
                    line.replace('duty_change_refused=1','duty_change_refused=0')]:
            with self.assertRaises(ValueError):verify_carrier_math(bad,required=True)


if __name__=='__main__':unittest.main()

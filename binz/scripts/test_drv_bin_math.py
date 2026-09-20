import unittest
from pathlib import Path
from drv_driven_handoff import verify_bin_math


class BinMathTests(unittest.TestCase):
    def test_retained_boundary_hold_and_recovery(self):
        from drv_timing_report import report
        root=Path(__file__).resolve().parents[1]/'captures'
        for name,reentry,commit,com in [('binmath_reentry68_30s_01.txt',True,37,63),
                                       ('binmath_hold69_01.txt',False,37,61)]:
            text=(root/name).read_text();verify_bin_math(text,required=True)
            result=report(text,reentry=reentry)
            self.assertEqual(result['commit_bracket_max_us'],commit)
            self.assertEqual(result['com_bracket_max_us'],com)

    def test_strict_optional_provenance(self):
        line='BINMATH adc_phase_divide=0 timeline_index_divide=0 exact=1 timestamps_unchanged=1'
        verify_bin_math('')
        verify_bin_math(line,required=True)
        for bad in ['',line+'\n'+line,line+' extra=1',
                    line.replace('exact=1','exact=0'),
                    line.replace('timestamps_unchanged=1','timestamps_unchanged=0')]:
            with self.assertRaises(ValueError):verify_bin_math(bad,required=True)


if __name__=='__main__':unittest.main()

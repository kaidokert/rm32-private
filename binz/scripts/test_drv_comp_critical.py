import unittest
from unittest.mock import patch
from drv_comp_critical import decode,verify_check


class CriticalTests(unittest.TestCase):
    def test_retained_powered_results_do_not_hide_failure(self):
        from pathlib import Path
        from drv_driven_handoff import verify
        root=Path(__file__).resolve().parents[1]/'captures'
        passed=(root/'compcritical_reentry68_30s_01.txt').read_text()
        self.assertTrue(verify(passed,30000,dropout=True,reentry=True)['powered_recovery_verified'])
        self.assertEqual(decode(passed,True)['max_masked_body_us'],33)
        failed=(root/'compcritical_reentry69_30s_01.txt').read_text()
        self.assertEqual(decode(failed,True)['max_masked_body_us'],48)
        with self.assertRaisesRegex(ValueError,'initial segment stopped before dropout'):
            verify(failed,30000,dropout=True,reentry=True)

    def test_retained_disabled_wrapper(self):
        from pathlib import Path
        text=(Path(__file__).resolve().parents[1]/'captures/compcritical_wrapper02.txt').read_text()
        result=verify_check(text)
        self.assertEqual(result['maxima_us'],[1,7,6,68])
        self.assertFalse(result['powered_wcet_proven'])

    def test_strict_service_contract(self):
        line='COMPCRITICAL calls=100 max_us=30 refused=0 filter_required=12 limit_us=60 restores_between_calls=1 recorder_inside=1'
        self.assertEqual(decode(line,True)['max_masked_body_us'],30)
        self.assertIsNone(decode(''))
        for bad in ['',line+'\n'+line,line.replace('calls=100','calls=0'),
                    line.replace('max_us=30','max_us=61'),line.replace('refused=0','refused=1'),
                    line.replace('recorder_inside=1','recorder_inside=0')]:
            with self.assertRaises(ValueError):decode(bad,True)

    def test_wrapper_checks_are_not_powered_duration_proof(self):
        body='\n'.join(f'CRITICALCHECK mode={i} passed=16 total=16 max_us={n} invalid_filters_refused=1 restored=1 disabled=1 wrapper_only=1 gate_authority=0' for i,n in enumerate([1,3,3,62]))+'\nFINALOFF\n'
        with patch('drv_comp_critical.verify_off'):
            self.assertFalse(verify_check(body)['powered_wcet_proven'])
            for bad in [body.replace('restored=1','restored=0'),body.replace('max_us=62','max_us=60'),
                        body.replace('passed=16','passed=15'),body+body]:
                with self.assertRaises(ValueError):verify_check(bad)


if __name__=='__main__':unittest.main()

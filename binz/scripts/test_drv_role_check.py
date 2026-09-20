"""Synthetic protocol tests, not hardware qualification."""
import unittest
from pathlib import Path
from drv_role_check import verify, verify_mode


class RoleCheckTests(unittest.TestCase):
    def good(self):
        # Reuse only an actual final-off grammar, never claim synthetic rows ran.
        capture = Path(__file__).resolve().parents[1] / 'captures/cpu_union_overhead_03.txt'
        final = capture.read_text().split('FINALOFF\n')[-1]
        rows = ''.join(f'ROLECHECKPART step={i} flags=127 elapsed_us=10 delta_ticks=640 error_mod=0\n' for i in range(1, 7))
        return rows + 'ROLECHECK passed=6 expected=6 restored=1 disabled=1 motor_authority=0\nFINALOFF\n' + final

    def test_valid(self):
        self.assertEqual(len(verify(self.good())), 6)

    def test_20k_period_does_not_alias_24k(self):
        text='ROLECHECKPERIOD ticks=3200 timer_hz=64000000 restored_ticks=6400\n'+self.good()
        self.assertEqual(len(verify(text,period_ticks=3200)),6)
        with self.assertRaises(ValueError):verify(text,period_ticks=2666)
        marker='PWMROLES bemf_only=1 equal_ccr=1 initial_ug=1 per_com_ug=0 forced_startup_unchanged=1 carrier_hz=20000\n'
        self.assertTrue(verify_mode(marker,carrier_hz=20000))
        with self.assertRaises(ValueError):verify_mode(marker,carrier_hz=24006)

    def test_mode_requires_exact_unique_provenance(self):
        marker = 'PWMROLES bemf_only=1 equal_ccr=1 initial_ug=1 per_com_ug=0 forced_startup_unchanged=1 carrier_hz=10000\n'
        self.assertTrue(verify_mode(marker, required=True))
        self.assertFalse(verify_mode('legacy capture'))
        self.assertTrue(verify_mode(marker.replace('10000','24006'),carrier_hz=24006))
        self.assertTrue(verify_mode(marker.replace('10000','32000'),carrier_hz=32000))
        self.assertTrue(verify_mode(marker.replace('10000','40000'),carrier_hz=40000))
        with self.assertRaises(ValueError):
            verify_mode(marker,carrier_hz=24006)
        for bad in ['', marker*2, marker.replace('per_com_ug=0','per_com_ug=1'),
                    marker.replace('10000','24000')]:
            with self.assertRaises(ValueError):
                verify_mode(bad, required=True)

    def test_period_specific_counter_and_restore_metadata(self):
        text='ROLECHECKPERIOD ticks=2666 timer_hz=64000000 restored_ticks=6400\n'+self.good()
        self.assertEqual(len(verify(text,period_ticks=2666)),6)
        for bad in [text.replace('2666','2667'),text.replace('restored_ticks=6400','restored_ticks=2666'),
                    text.replace('elapsed_us=10','elapsed_us=40'),text+text.splitlines()[0]+'\n']:
            with self.assertRaises((ValueError,RuntimeError)):
                verify(bad,period_ticks=2666)
        with self.assertRaises(ValueError):
            verify(self.good(),period_ticks=2666)

    def test_refuses_corruption_failure_and_missing_safing(self):
        good = self.good()
        for bad in [good.replace('flags=127', 'flags=126', 1),
                    good.replace('step=6', 'step=5'),
                    good.replace('delta_ticks=640', 'delta_ticks=2000', 1),
                    good.replace('elapsed_us=10', 'elapsed_us=100', 1),
                    good.replace('restored=1', 'restored=0'),
                    good.replace('motor_authority=0', 'motor_authority=1'),
                    good.replace('error_mod=0', 'error_mod=6400', 1),
                    good[:good.index('FINALOFF')],
                    good + 'ROLECHECK bad\n', good + 'FINALOFF\n']:
            with self.subTest(bad=bad[:100]):
                with self.assertRaises((ValueError, RuntimeError)):
                    verify(bad)


if __name__ == '__main__':
    unittest.main()

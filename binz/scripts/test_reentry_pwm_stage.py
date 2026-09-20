import unittest
from scripts.drv_reentry_pwm_stage import verify


class PwmStage(unittest.TestCase):
    def test_next_edge_identity_and_refusals(self):
        base = ('RECOVERPWMSTAGE enabled=1 before_sensing=1 physical_revalidate=1\n'
                'RECOVERGUARDSTAGE enabled=1 fresh_admission=1\n'
                'RECOVERYACQ result=1 step=6 interval_ticks=1000 elapsed_us=6500 '
                'intervals=12 max_gap_ticks=160 disabled=0 gate_authority=0\n')
        row = ('FOLLOWEDGE result=1 prior_step=6 prior_tick=12000 step=1 onset=13000 '
               'confirmed=13040 interval=1000 max_gap=0 half_us=1\n')
        verify(base + row, True, True, True)
        stage = 'FOLLOWSTAGE stage=5\n'
        verify(base + row + stage, True, True, True)
        verify(base + row + stage + 'FOLLOWREADS count=2\n', True, True, True)
        for captured in range(1,7):
            verify(base + row + stage + f'FOLLOWREADS count=0 captured={captured}\n', True, True, True)
        for captured in (0,7,4294967296):
            with self.assertRaises(ValueError):
                verify(base + row + stage + f'FOLLOWREADS count=0 captured={captured}\n', True, True, True)
        for counter in ['FOLLOWREADS count=2\n'*2, 'FOLLOWREADS count=4294967296\n']:
            with self.assertRaises(ValueError):
                verify(base + row + counter, True, True, True)
        mode = 'FOLLOWMODE expected_phase_only=1\n'
        verify(base + row + stage + mode, True, True, True, True)
        for text, selected in [(base+row+mode, False), (base+row, True),
                               (base+row+mode*2, True)]:
            with self.assertRaises(ValueError):
                verify(text, True, True, True, selected)
        for bad_stage in [stage * 2, stage.replace('stage=5', 'stage=1'),
                          stage.replace('stage=5', 'stage=9')]:
            with self.assertRaises(ValueError):
                verify(base + row + bad_stage, True, True, True)
        verify(base + row.replace('12000', '4294966796').replace('13000', '500')
               .replace('13040', '540'), True, True, True)
        for bad in [row * 2, '', row.replace('result=1', 'result=5'),
                    row.replace('step=1 onset', 'step=2 onset'),
                    row.replace('prior_step=6', 'prior_step=5'),
                    row.replace('interval=1000', 'interval=999'),
                    row.replace('confirmed=13040', 'confirmed=13039'),
                    row.replace('confirmed=13040', 'confirmed=13241'),
                    row.replace('max_gap=0', 'max_gap=201'),
                    row.replace('onset=13000', 'onset=4294980296')]:
            with self.subTest(bad=bad), self.assertRaises(ValueError):
                verify(base + bad, True, True, True)
        with self.assertRaises(ValueError):
            verify(base + row, True, True)
        with self.assertRaises(ValueError):
            verify(base + row, True, False, True)

    def test_guard_stage_must_be_declared_with_pwm(self):
        pwm = 'RECOVERPWMSTAGE enabled=1 before_sensing=1 physical_revalidate=1\n'
        guard = 'RECOVERGUARDSTAGE enabled=1 fresh_admission=1\n'
        verify(pwm + guard, True, True)
        for text, required, staged in [(pwm, True, True), (pwm + guard, True, False),
                                      (guard, False, True), (pwm + guard * 2, True, True),
                                      (pwm + guard.replace('fresh_admission=1', 'fresh_admission=0'), True, True)]:
            with self.assertRaises(ValueError):
                verify(text, required, staged)
    def test_explicit_candidate_only(self):
        marker = 'RECOVERPWMSTAGE enabled=1 before_sensing=1 physical_revalidate=1\n'
        verify('', False)
        verify(marker, True)
        for text, required in [('', True), (marker, False), (marker * 2, True),
                               (marker.replace('physical_revalidate=1', 'physical_revalidate=0'), True)]:
            with self.assertRaises(ValueError):
                verify(text, required)

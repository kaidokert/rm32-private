"""Static ownership checks for the opt-in average-current foldback path."""
from pathlib import Path
import sys
import unittest


ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / 'scripts'))
from live_armed_baseline import current_foldback_report


def compact(path: Path) -> str:
    return ''.join(path.read_text().split())


class CurrentFoldbackWiring(unittest.TestCase):
    def test_feature_keeps_live_control_and_average_protection(self):
        cargo = compact(ROOT / 'Cargo.toml')
        self.assertIn(
            'bench-current-foldback=["bench-current-1500","bench-current-foldback-policy"]',
            cargo,
        )
        self.assertIn('bench-current-foldback-2000=["bench-current-2000","bench-current-foldback-policy"]', cargo)
        self.assertIn('bench-current-foldback-2500=["bench-current-2500","bench-current-foldback-policy"]', cargo)
        self.assertIn('bench-current-foldback-3500=["bench-current-3500","bench-current-foldback-policy"]', cargo)
        self.assertIn('bench-duty-50=["bench-current-foldback-policy"]', cargo)

    def test_dma_only_publishes_through_current_policy(self):
        dma = compact(ROOT / 'examples/support/adc_stream.rs')
        self.assertIn('average_current_live::scan_raw(', dma)
        self.assertNotIn('update_live_duty', dma)
        self.assertNotIn('current_foldback::', dma)

    def test_foreground_owns_foldback_and_all_upward_requests_are_capped(self):
        shell = compact(ROOT / 'examples/shell-pwm.rs')
        self.assertIn('average_current_live::take_foldback_warning()', shell)
        self.assertIn('current_governor.warning(current,warning.residual,warning.allowance,)', shell)
        self.assertIn('powered_timer::update_live_duty(request)', shell)
        self.assertIn('current_governor.request(dasu32)', shell)
        self.assertIn('current_governor.request(duty)', shell)
        self.assertIn('normal_restart_resume_target=duty', shell)
        self.assertIn('average_current_live::record_foldback(reduction.duty,reduction.step,)', shell)

    def test_warning_is_latched_until_foreground_consumes_it(self):
        live = compact(ROOT / 'examples/support/average_current_live.rs')
        scan = live.split('fnscan_inner', 1)[1].split('pubfntake_foldback_warning', 1)[0]
        self.assertIn('ifletSome(value)=residual.filter(|_|limit.over_streak()==1){', scan)
        self.assertIn('FOLDBACK_PENDING.fetch_max(valueasu32,', scan)
        self.assertNotIn('FOLDBACK_PENDING.store(0,', scan)
        self.assertIn('FOLDBACK_PENDING.swap(0,', live)

    def test_capture_summary_requires_exact_monotonic_derating(self):
        row = lambda count, last: (
            f'CURRENTFOLDBACK count={count} last_duty_tenths={last} '
            'step_tenths=50 release=none first_over_warning=1 '
            'second_over_stop=1 foreground_writer=1\n'
        )
        self.assertEqual(current_foldback_report(row(0, 0), 300)['effective_duty'], 300)
        self.assertEqual(current_foldback_report(row(1, 250), 300)['effective_duty'], 250)
        self.assertEqual(current_foldback_report(row(1, 450), 500)['effective_duty'], 450)
        for text in [row(0, 250), row(1, 260), row(1, 300), row(1, 350), '']:
            with self.assertRaises(RuntimeError):
                current_foldback_report(text, 300)

        fine = lambda count, last: (
            f'CURRENTFOLDBACK count={count} last_duty_tenths={last} '
            'step_tenths=10 release=none first_over_warning=1 '
            'unacknowledged_second_over_stop=1 foreground_writer=1\n'
        )
        self.assertEqual(current_foldback_report(fine(1, 390), 400)['effective_duty'], 390)
        self.assertEqual(current_foldback_report(fine(3, 370), 400)['effective_duty'], 370)

        adaptive = lambda count, last, step: (
            f'CURRENTFOLDBACK count={count} last_duty_tenths={last} '
            f'last_step_tenths={step} step_policy=severity10_50 release=none '
            'first_over_warning=1 unacknowledged_second_over_stop=1 foreground_writer=1\n'
        )
        report=current_foldback_report(adaptive(2,350,50),400)
        self.assertEqual((report['effective_duty'],report['step_policy']),(350,'adaptive'))
        self.assertEqual(current_foldback_report(adaptive(0,0,0),400)['effective_duty'],400)
        for text in [adaptive(0,0,10),adaptive(1,390,0),adaptive(1,390,60)]:
            with self.assertRaises(RuntimeError):current_foldback_report(text,400)


if __name__ == '__main__':
    unittest.main()

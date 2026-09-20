import unittest
from pathlib import Path
from live_armed_baseline import lean_exploration_report,normal_restart_report,normal_restart_compact_report

class LeanReport(unittest.TestCase):
    def test_compact_restart_requires_measured_second_deadline(self):
        root=Path(__file__).resolve().parents[1]
        text=(root/'captures/reverse_direction_2026-09-18_hybrid_restart10_first_pass.txt').read_text().replace('\r','')
        marker='NORMALRESTART4 planned_remaining_us=14236900 actual_second_power_us=14236004 window_rounding_us=1000 postrun_only=1\n'
        marked=text.replace('NORMALRESTART3 resume_target=', marker+'NORMALRESTART3 resume_target=', 1)
        self.assertEqual(normal_restart_compact_report(marked, expected_resume=100)['remaining_powered_us'], 14236004)
        for bad in (text, marked.replace('actual_second_power_us=14236004', 'actual_second_power_us=14235004'),
                    marked.replace('handoff=1', 'handoff=0', 1)):
            with self.assertRaises(RuntimeError):
                normal_restart_compact_report(bad, expected_resume=100)

    def test_off_capture_is_not_a_motor_qualification(self):
        root=Path(__file__).resolve().parents[1]
        text=(root/'captures/notrace_708_guard.txt').read_text().replace('\r','')
        boundary='FINALOFF\n'
        self.assertIn(boundary,text)
        r=lean_exploration_report(text)
        self.assertTrue(r['outputs_off_verified'])
        self.assertFalse(r['qualification'])
        self.assertFalse(r['lean_summary_seen'])
        for bad in [text.replace(boundary,''),text+boundary,text.replace('en=0','en=1')]:
            with self.assertRaises((ValueError,RuntimeError)):lean_exploration_report(bad)

    def test_restart_requires_second_powered_deadline(self):
        root=Path(__file__).resolve().parents[1]
        passed=(root/'captures/m0clean_765_direct200_normalrestart_30s_02.txt').read_text().replace('\r','')
        r=normal_restart_report(passed)
        self.assertTrue(r['fresh_transfer'])
        self.assertEqual(r['remaining_powered_us'],22979005)
        for name in ('m0clean_765_direct200_ramp20_normalrestart_30s_01.txt',
                     'm0clean_765_direct200_normalrestart_30s_03.txt'):
            failed=(root/'captures'/name).read_text().replace('\r','')
            with self.assertRaises(RuntimeError):normal_restart_report(failed)

    def test_new_outcome_marker_is_mandatory_for_live_campaign(self):
        root=Path(__file__).resolve().parents[1]
        passed=(root/'captures/m0clean_765_direct200_normalrestart_30s_02.txt').read_text().replace('\r','')
        with self.assertRaises(RuntimeError):normal_restart_report(passed,require_outcome=True)
        marked=passed.replace('NORMALRESTART result=',
            'NORMALRESTART2 handoff=1 final_drive_reason=22 final_power_reason=2 final_power_stop_us=22979005 settle_us=1000000 settings_replayed=1 outputs_disabled=1\nNORMALRESTART result=',1)
        self.assertTrue(normal_restart_report(marked,require_outcome=True)['deadline_complete'])

    def test_requested_live_duty_requires_exact_restoration_marker(self):
        root=Path(__file__).resolve().parents[1]
        passed=(root/'captures/m0clean_770_direct200_normalrestart_30s_01.txt').read_text().replace('\r','')
        marker='NORMALRESTART3 resume_target=200 resume_applied=200 resume_steps=3 step_tenths=50 period_us=2000000 foreground_only=1\n'
        marked=passed.replace('NORMALRESTART result=',marker+'NORMALRESTART result=',1)
        r=normal_restart_report(marked,require_outcome=True,expected_resume=200)
        self.assertEqual(r['resume'],(200,200,3))
        for bad in [passed,marked.replace('resume_applied=200','resume_applied=170'),
                    marked.replace('resume_steps=3','resume_steps=0')]:
            with self.assertRaises(RuntimeError):
                normal_restart_report(bad,require_outcome=True,expected_resume=200)

if __name__=='__main__':unittest.main()

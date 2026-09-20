"""Retained hardware evidence plus deliberate malformed-summary tests."""
import unittest
from pathlib import Path

from drv_timing_report import report, acquire_stages


class TimingReportTests(unittest.TestCase):
    root = Path(__file__).resolve().parents[1] / 'captures'

    def test_retained_acquisition_timing_probe(self):
        text=(self.root/'acquiretiming_reentry64_30s_01.txt').read_text()
        result=report(text,reentry=True)
        self.assertEqual(list(result['acquisition_stage_timing']['brackets_us'].values()),
                         [36,5,3,2,5])
        self.assertEqual(result['seed_stage_timing']['arm_age_us'],94)
        self.assertEqual(result['segment_arm_spare_above_floor_us'],17.5)
        self.assertFalse(result['acquisition_stage_timing']['exclusive_cost'])

    def test_retained_seed_division_timing(self):
        from drv_driven_handoff import verify_seed_math
        text=(self.root/'seeddiv_reentry64_30s_01.txt').read_text()
        verify_seed_math(text,required=True)
        result=report(text,reentry=True)
        self.assertEqual(list(result['acquisition_stage_timing']['brackets_us'].values()),
                         [31,6,3,2,5])
        self.assertEqual(result['seed_stage_timing']['arm_age_us'],89)
        self.assertEqual(result['segment_arm_spare_above_floor_us'],24)

    def test_optional_acquisition_stages_and_refusals(self):
        stages={'brackets_us':{'edge_to_entry':49}}
        line='ACQUIRELAT edge_ticks=100 qualified_ticks=144 cleared_ticks=164 published_ticks=180 returned_ticks=190 half_us=1 recovery_only=1'
        result=acquire_stages(line,stages,reentry=True)
        self.assertEqual(list(result['brackets_us'].values()),[22,10,8,5,4])
        self.assertFalse(result['removable_delay_proven'])
        self.assertIsNone(acquire_stages('',stages,reentry=True))
        for bad in [line+'\n'+line,line.replace('144','138'),line.replace('164','142'),
                    line.replace('190','200'),line.replace('190','40000'),
                    line+' extra=1','ACQUIRELAT',line.replace('half_us=1','half_us=0')]:
            with self.assertRaises(ValueError):acquire_stages(bad,stages,reentry=True)
        with self.assertRaises(ValueError):acquire_stages(line,None,reentry=True)
        with self.assertRaises(ValueError):acquire_stages(line,stages,reentry=False)

    def test_recovered_segment_and_original_arm_are_distinct(self):
        for n, spare, deadline in [(1, 3, 145), (2, 2, 198), (3, 3, 232)]:
            text = (self.root / f'driven_reserve45_{n:02}.txt').read_text()
            result = report(text, reentry=True)
            self.assertEqual(result['segment_arm_spare_above_floor_us'], spare)
            self.assertEqual(result['original_deadline_spare_us'], deadline)
            self.assertGreater(result['initial_arm_spare_above_floor_us'], 20)
            self.assertIsNone(result['cpu_utilization_percent'])
            self.assertFalse(result['full_isr_entry_exit_measured'])
            self.assertFalse(result['faster_operation_qualified'])
            self.assertTrue(12000 < result['comp_calls_per_second'] < 14000)

    def test_initial_segment(self):
        result = report((self.root / 'driven_align45_01.txt').read_text())
        self.assertEqual(result['segment'], 'initial')
        self.assertEqual(result['segment_arm_remaining_us'], result['initial_arm_remaining_us'])
        self.assertIsNone(result['original_deadline_spare_us'])

    def test_timing_summaries_must_be_unique_real_and_complete(self):
        text = (self.root / 'driven_reserve45_01.txt').read_text()
        line = next(s for s in text.splitlines() if s.startswith('COREEXTI '))
        for bad in [text + '\n' + line + '\n',
                    text.replace(line, ''),
                    text.replace(line, line.replace('live=1', 'live=0')),
                    text.replace(line, line.replace('synthetic=0', 'synthetic=1')),
                    text.replace(line, line + ' irq_calls=1'),
                    text.replace('comp_max_us=82', 'missing_max=82')]:
            with self.assertRaises(ValueError):
                report(bad, reentry=True)

    def test_known_late_recovery_remains_failure(self):
        with self.assertRaises(ValueError):
            report((self.root / 'driven_reentry45_04.txt').read_text(), reentry=True)


if __name__ == '__main__':
    unittest.main()

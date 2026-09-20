from pathlib import Path
import sys
import unittest

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / 'scripts'))
from drv_prepared_timer_check import verify, verify_load


class PreparedTimerCheck(unittest.TestCase):
    def test_priority_identity_is_required_for_ab(self):
        for priority,late in [(128,23),(0,4)]:
            text=(ROOT/f'captures/preparedpriority_664_p{priority}.txt').read_text()
            result=verify_load(text,priority)
            self.assertEqual(result['max_late_us'],late)
            with self.assertRaises(ValueError):verify_load(text,128-priority)
            with self.assertRaises(ValueError):verify_load(text.replace(f' priority={priority}',''),priority)

    def test_loaded_result_is_not_a_timing_qualification(self):
        text=(ROOT/'captures/preparedload_663_loaded.txt').read_text()
        result=verify_load(text)
        self.assertEqual(result['max_late_us'],24)
        self.assertFalse(result['timing_qualified'])
        for before,after in [('events=32','events=31'),('refusal=0','refusal=6'),
                             ('min_feedback=3','min_feedback=0'),('failures=0','failures=1')]:
            with self.assertRaises(ValueError):verify_load(text.replace(before,after))
    def test_mapped_capture_requires_mapping_evidence(self):
        text=(ROOT/'captures/preparedmap_662_disabled.txt').read_text()
        self.assertEqual(verify(text)['accepted'],64)
        for before,after in [('max_mapping_us=2','max_mapping_us=9'),
                             ('PREPAREDMAP trials=32','PREPAREDMAP trials=31'),
                             ('quantization_bound_ticks=4','quantization_bound_ticks=0')]:
            with self.assertRaises(ValueError):verify(text.replace(before,after))

    def test_retained_pass_and_failure(self):
        good = (ROOT / 'captures/preparedtimer_660b_disabled.txt').read_text()
        self.assertEqual(verify(good)['max_publish_us'], 3)
        bad = (ROOT / 'captures/preparedtimer_660_disabled.txt').read_text()
        with self.assertRaises(ValueError): verify(bad)
        for before, after in [('max_publish_us=3', 'max_publish_us=9'),
                              ('deadline_error_us=1', 'deadline_error_us=4'),
                              ('refused=128', 'refused=127'),
                              ('irq_masked=1', 'irq_masked=0')]:
            with self.assertRaises(ValueError): verify(good.replace(before, after))


if __name__ == '__main__': unittest.main()

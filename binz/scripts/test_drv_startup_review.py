import tempfile
import unittest
from pathlib import Path

from drv_startup_review import acquisition_context


class AcquisitionContextTests(unittest.TestCase):
    def context(self, head, count=256, header_count=256):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / 'capture.csv'
            path.with_suffix('.txt').write_text(
                f'CAP n={header_count} capacity=256 head={head} '
                'adc_order=rotating_ABC_BCA_CAB vcal=1662 reason=4\n')
            return acquisition_context(path, count, dict(
                ia=2048, ib=2048, ic=3300, theta=164, on_a=1, on_b=3, on_c=7))

    def test_ring_wrap_is_not_global_tick_rotation(self):
        self.assertEqual(self.context(0)['final_adc_order'], 'ABC')  # slot255
        self.assertEqual(self.context(1)['final_adc_order'], 'ABC')  # slot0
        self.assertEqual(self.context(2)['final_adc_order'], 'BCA')

    def test_actual_failures_have_different_scan_ordinals(self):
        self.assertEqual(self.context(73)['peak_phase_scan_ordinal'], 3)
        self.assertEqual(self.context(123)['peak_phase_scan_ordinal'], 1)

    def test_header_csv_mismatch_rejected(self):
        with self.assertRaises(ValueError):
            self.context(73, count=255)

    def test_no_transcript_no_invented_order(self):
        with tempfile.TemporaryDirectory() as directory:
            self.assertIsNone(acquisition_context(Path(directory) / 'absent.csv', 1, {}))


if __name__ == '__main__':
    unittest.main()

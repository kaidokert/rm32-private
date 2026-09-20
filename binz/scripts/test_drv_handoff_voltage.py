import unittest
from pathlib import Path
from drv_handoff_voltage import audit


class HandoffVoltageTests(unittest.TestCase):
    def test_existing_waveform_all_phase_bins(self):
        source = (Path(__file__).resolve().parents[1]/'examples/support/sine_table.rs').read_text()
        result = audit(source)
        self.assertEqual(result['phase_bins'], 256)
        self.assertEqual(result['sixstep_pair_ccr'], 512)
        self.assertGreater(result['pair_voltage_ratio_range'][0], 1.4)
        reduced = audit(source, 65, 65)
        self.assertLess(reduced['pair_voltage_ratio_range'][1],
                        result['pair_voltage_ratio_range'][1])

    def test_missing_lut_is_not_invented(self):
        with self.assertRaises(ValueError):
            audit('')

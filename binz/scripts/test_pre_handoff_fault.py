from pathlib import Path
import sys
import unittest
sys.path.insert(0,str(Path(__file__).resolve().parent))
from drv_driven_handoff import reject_pre_handoff_fault


class PreHandoffFault(unittest.TestCase):
    def test_rate_refusal_before_downstream_provenance(self):
        root=Path(__file__).resolve().parents[1]
        for name in ['startup_direct_677_80_30s.txt','rate_679_direct_80_30s.txt']:
            text=(root/'captures'/name).read_text()
            with self.assertRaisesRegex(ValueError,'initial qualification IRQ-rate refusal'):
                reject_pre_handoff_fault(text)
            with self.assertRaises(ValueError):
                reject_pre_handoff_fault(text[:text.rfind('FINALOFF\n')])
    def test_retained_startup_stop_is_not_a_build_mismatch(self):
        root=Path(__file__).resolve().parents[1]
        text=(root/'captures/setup_phase_676_80_30s_03.txt').read_text()
        with self.assertRaisesRegex(ValueError,'startup current ADC rail/peak stop'):
            reject_pre_handoff_fault(text)
        with self.assertRaises(ValueError):
            reject_pre_handoff_fault(text[:text.rfind('FINALOFF\n')])
        passing=(root/'captures/setup_phase_676_80_30s.txt').read_text()
        self.assertIsNone(reject_pre_handoff_fault(passing))

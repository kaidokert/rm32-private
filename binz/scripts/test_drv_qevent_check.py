from pathlib import Path
import unittest
from drv_qevent_check import verify


class EventCheckTests(unittest.TestCase):
    def test_retained_hardware_cost_failure_stays_rejected(self):
        root=Path(__file__).resolve().parents[1]
        with self.assertRaisesRegex(ValueError,'7>2'):
            verify((root/'captures/qevent_493_scope01.txt').read_text())

    def sample(self):
        root=Path(__file__).resolve().parents[1]
        final=(root/'captures/seedmask_488_guard01.txt').read_text().split('FINALOFF\n')[-1]
        rows=[f'QEVENTCHECK preload={p} mode={m} passed=16 total=16 max_us={4 if m in (2,3,4) else 2} scope_only=1 gate_authority=0'
              for p in (0,16,32) for m in range(6)]
        return '\n'.join(rows)+'\nQEVENTCHECK END disabled=1\nFINALOFF\n'+final
    def test_fixed_cost_gates(self):
        text=self.sample()
        self.assertTrue(verify(text)['disabled_cost_gates_passed'])
        for bad in [text.replace('max_us=2','max_us=3',1),
                    text.replace('max_us=4','max_us=5',1),
                    text.replace('passed=16','passed=15',1),
                    text.replace('preload=32','preload=16',1),
                    text+'FINALOFF\n']:
            with self.assertRaises(ValueError):verify(bad)

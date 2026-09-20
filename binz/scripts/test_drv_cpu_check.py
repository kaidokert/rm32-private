"""Idle overhead gate tests; retained failed measurements remain failures."""
import unittest
from pathlib import Path
from drv_cpu_check import verify
from drv_capture import verify_off


class CpuOverheadTests(unittest.TestCase):
    root = Path(__file__).resolve().parents[1] / 'captures'
    def test_union_gate_requires_protocol_and_preserves_previous_failure(self):
        good = (self.root / 'cpu_union_overhead_03.txt').read_text()
        self.assertEqual([r['max_pair_us'] for r in verify(good, union=True)], [2, 7, 10])
        for bad in [good.replace('CPUCHECKTYPE union=1', ''),
                    good.replace('CPUCHECKTYPE union=1', 'CPUCHECKTYPE union=0'),
                    good + '\nCPUCHECKTYPE union=1\n',
                    (self.root / 'cpu_union_overhead_02.txt').read_text()]:
            with self.assertRaises(ValueError):
                verify(bad, union=True)

    def test_two_real_overhead_failures_have_final_off(self):
        for name in ['cpu_overhead_01.txt', 'cpu_overhead_03.txt',
                     'cpu_overhead_05.txt', 'cpu_overhead_07.txt']:
            text = (self.root / name).read_text()
            verify_off(text[text.rfind('FINALOFF'):].encode())
            with self.assertRaisesRegex(ValueError, 'overhead gate failed'):
                verify(text)

    def test_synthetic_gate_and_missing_readback(self):
        text = (self.root / 'cpu_overhead_03.txt').read_text()
        # Artificial timing edits exercise policy, NOT hardware qualification.
        text = text.replace('sum_us=4240 max_us=17', 'sum_us=2000 max_us=10')
        self.assertEqual(len(verify(text)), 3)
        for bad in [text.replace('mode=2', 'mode=1'), text.replace('fault=0', 'fault=1'),
                    text[:text.rfind('FINALOFF')], text.replace('sum_us=2000', 'sum_us=3000')]:
            with self.assertRaises((ValueError, RuntimeError)):
                verify(bad)

    def test_uart_silent_attempt_cannot_pass(self):
        for name in ['cpu_overhead_02.txt', 'cpu_overhead_04.txt', 'cpu_overhead_06.txt']:
            with self.assertRaises((ValueError, RuntimeError)):
                verify((self.root / name).read_text())


if __name__ == '__main__':
    unittest.main()

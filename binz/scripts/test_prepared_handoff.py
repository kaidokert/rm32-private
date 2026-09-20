"""Prototype policy semantics and M0 codegen; no hardware timing authority."""
from pathlib import Path
import subprocess
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[1]


class PreparedHandoff(unittest.TestCase):
    def test_policy_and_m0_codegen(self):
        source = ROOT / 'examples/support/prepared_handoff.rs'
        with tempfile.TemporaryDirectory() as directory:
            temp = Path(directory)
            exe, asm = temp / 'policy.exe', temp / 'policy.s'
            subprocess.run(['rustc', '--test', str(source), '-o', str(exe)], check=True)
            subprocess.run([str(exe)], check=True)
            subprocess.run(['rustc', '--crate-type', 'lib', '--target', 'thumbv6m-none-eabi',
                            '-C', 'opt-level=s', '-C', 'panic=abort', '--emit=asm',
                            '-o', str(asm), str(source)], check=True)
            emitted = asm.read_text()
            for name in ('publish', 'service', 'map_deadline'):
                self.assertIn(name, emitted)  # never pass an empty/dead-code listing
            self.assertNotRegex(emitted, r'\b(?:bl|blx|udiv|sdiv)\b|__aeabi')
            self.assertIn('.code\t16', emitted)


if __name__ == '__main__':
    unittest.main()

"""Exercise actual register-sequence modules, without hardware timing claims."""
from pathlib import Path
import subprocess
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[1]


class RecoveryPwmPrepare(unittest.TestCase):
    def test_disabled_sequence(self):
        modules = ['sixstep', 'phase_gpio_plan', 'carrier_profile', 'duty_envelope', 'live_duty', 'phase_role_sequence']
        source = '\n'.join(
            f'#[path="{(ROOT / "examples/support" / (name + ".rs")).as_posix()}"] mod {name};'
            for name in modules)
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory)
            (path / 'test.rs').write_text(source)
            for high in (False, True):
                exe = path / ('test50.exe' if high else 'test30.exe')
                flags = ['--cfg', 'feature="bench-recovery-duty-check"']
                if high:
                    flags += ['--cfg', 'feature="bench-duty-50"']
                subprocess.run(['rustc', '--edition=2021', '--test', *flags,
                                str(path / 'test.rs'), '-o', str(exe)], check=True)
                subprocess.run([str(exe)], check=True)


if __name__ == '__main__':
    unittest.main()

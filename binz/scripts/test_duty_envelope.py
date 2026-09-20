"""Compile the actual live parser/governor/restart policy at both campaign caps."""
from pathlib import Path
import subprocess
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[1]


class DutyEnvelope(unittest.TestCase):
    def test_actual_modules_at_30_and_50_percent(self):
        modules = ['duty_envelope', 'live_command', 'current_foldback', 'normal_restart', 'nominal_current', 'average_current', 'rolling_current', 'bus_foldback', 'bus_sag']
        source = '\n'.join(
            f'#[path="{(ROOT / "examples/support" / (name + ".rs")).as_posix()}"] mod {name};'
            for name in modules
        )
        with tempfile.TemporaryDirectory(prefix='binz-duty-envelope-') as directory:
            path = Path(directory)
            src = path / 'test.rs'
            src.write_text(source)
            cases = ((False, 300, None, False, False), (True, 500, 2500, False, False),
                     (True, 500, 3500, False, False), (True, 500, 3500, True, False),
                     (True, 500, 3500, False, True))
            for high, maximum, current, fast, recovery in cases:
                exe = path / f'test{maximum}-{current or 1000}-{int(fast)}-{int(recovery)}.exe'
                flags = (['--cfg', 'feature="bench-duty-50"',
                          '--cfg', f'feature="bench-current-{current}"'] if high else [])
                if fast:
                    flags += ['--cfg', 'feature="bench-current-fast-20"']
                if recovery:
                    flags += ['--cfg', 'feature="bench-bus-recovery"']
                subprocess.run(
                    ['rustc', '--edition=2021', '--test', *flags, str(src), '-o', str(exe)],
                    check=True,
                )
                subprocess.run([str(exe)], check=True)


if __name__ == '__main__':
    unittest.main()

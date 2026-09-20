"""Host-test the real speed-scaled accepted-event watchdog policy."""
from pathlib import Path
import subprocess


if __name__ == "__main__":
    root = Path(__file__).resolve().parents[1]
    executable = root / "target/speed_event_watch_test.exe"
    subprocess.run(
        [
            "rustc",
            "--edition=2021",
            "--cfg",
            'feature="bench-speed-event-watch"',
            str(root / "scripts/speed_event_watch_test.rs"),
            "-o",
            str(executable),
        ],
        cwd=root,
        check=True,
    )
    subprocess.run([str(executable)], cwd=root, check=True)

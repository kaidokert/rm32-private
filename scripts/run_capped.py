#!/usr/bin/env python3
"""Run a command under a hard wall-clock cap; kill its whole process tree
on expiry.

    python scripts/run_capped.py [--secs 120] -- <command> [args...]

Exit code: the command's own, or 124 on timeout (as coreutils `timeout`).
Used by the gc gate so a hung harness can never stall a session.
"""

from __future__ import annotations

import argparse
import os
import signal
import subprocess
import sys


def kill_tree(p: subprocess.Popen) -> None:
    if os.name == "nt":
        subprocess.run(["taskkill", "/F", "/T", "/PID", str(p.pid)],
                       capture_output=True)
    else:
        os.killpg(p.pid, signal.SIGKILL)


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--secs", type=float, default=120.0)
    ap.add_argument("cmd", nargs=argparse.REMAINDER)
    a = ap.parse_args()
    cmd = a.cmd[1:] if a.cmd[:1] == ["--"] else a.cmd
    if not cmd:
        ap.error("no command")
    kw = {} if os.name == "nt" else {"start_new_session": True}
    p = subprocess.Popen(cmd, **kw)
    try:
        return p.wait(timeout=a.secs)
    except subprocess.TimeoutExpired:
        kill_tree(p)
        p.wait()
        print(f"run_capped: TIMEOUT after {a.secs:.0f} s, killed: {' '.join(cmd)}",
              file=sys.stderr)
        return 124
    except KeyboardInterrupt:
        kill_tree(p)
        raise


if __name__ == "__main__":
    sys.exit(main())

#!/usr/bin/env python3
"""Passive live display window for the rinz scope.

Shows logs/latest_zc.png and reloads it whenever the file changes on disk. It
reads NO keyboard input and never needs focus -- pair it with scope_live_ui.py,
which owns the serial port, drives the firmware live stream ('l' / 'k') and keeps
regenerating latest_zc.png. Keep the terminal (scope_live_ui) focused for snappy
control; this window just mirrors the latest plot.

    # terminal 1 (control, responsive):
    python scripts/scope_live_ui.py COM41 --mode six-step --hz 120 --amp 15
    #   then press 'l' to start streaming, 'k' to stop
    # terminal 2 (display only):
    python scripts/scope_view.py
"""

from __future__ import annotations

import argparse
import sys
from pathlib import Path

import matplotlib

# An interactive backend is required for an on-screen window; pick the first that
# imports. (scope_common forces Agg, but this script does not import it.)
for _backend in ("TkAgg", "QtAgg", "Qt5Agg", "MacOSX"):
    try:
        matplotlib.use(_backend)
        break
    except Exception:
        continue

import matplotlib.image as mpimg  # noqa: E402
import matplotlib.pyplot as plt  # noqa: E402


def parse_args() -> argparse.Namespace:
    p = argparse.ArgumentParser(description="Passive auto-reloading viewer for latest_zc.png")
    p.add_argument("--path", type=Path, default=Path("logs/latest_zc.png"), help="image to display")
    p.add_argument("--interval", type=float, default=0.4, help="seconds between file-change checks")
    return p.parse_args()


def main() -> int:
    args = parse_args()

    fig, ax = plt.subplots(figsize=(14, 9))
    ax.axis("off")
    try:
        fig.canvas.manager.set_window_title(f"rinz live view — {args.path}")
    except Exception:
        pass

    running = {"on": True}
    fig.canvas.mpl_connect("close_event", lambda _evt: running.__setitem__("on", False))
    plt.show(block=False)

    im = None
    last_mtime = None
    last_size = -1
    waiting_printed = False
    while running["on"]:
        try:
            stat = args.path.stat()
            mtime, size = stat.st_mtime, stat.st_size
        except OSError:
            mtime = size = None

        if mtime is not None and (mtime, size) != (last_mtime, last_size):
            try:
                data = mpimg.imread(str(args.path))
                if im is None:
                    im = ax.imshow(data)
                else:
                    im.set_data(data)
                    im.set_extent((0, data.shape[1], data.shape[0], 0))
                ax.set_xlim(0, data.shape[1])
                ax.set_ylim(data.shape[0], 0)
                fig.canvas.draw_idle()
                last_mtime, last_size = mtime, size
            except Exception:
                # File mid-write (despite atomic replace) or unreadable; retry next tick.
                pass
        elif mtime is None and not waiting_printed:
            print(f"waiting for {args.path} ... (start streaming with 'l' in scope_live_ui)")
            waiting_printed = True

        plt.pause(args.interval)

    return 0


if __name__ == "__main__":
    sys.exit(main())

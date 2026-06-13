#!/usr/bin/env python3
"""Live streaming BEMF zero-crossing viewer for the rinz scope1 firmware.

Sends the firmware 'l' command (continuous back-to-back dumps) and renders each
fresh capture into a live matplotlib window, throttled to --refresh seconds.
Reuses the exact ZC plot from scope_common (render_zc_figure), so the live view
matches latest_zc.png.

Run this INSTEAD of scope_live_ui.py -- the serial port is exclusive:

    python scripts/scope_stream.py COM41 --hz 120 --amp 15

Close the window or press Ctrl-C to stop; the motor is killed on exit.

Note: the hex text dump is inefficient for 12-bit samples (~16 B/frame). At
115200 baud one 2-rev dump is ~0.5 s, so the firmware streams ~2 dumps/s and we
redraw at --refresh (default 1 s), dropping intermediate dumps. A binary/packed
mode would raise that ceiling later; this path is intentionally the same dump.
"""

from __future__ import annotations

import argparse
import sys
import time
from pathlib import Path

try:
    import serial
except ImportError as exc:
    raise SystemExit("missing dependency: pip install pyserial") from exc

sys.path.insert(0, str(Path(__file__).parent))
from scope_common import (
    AMP_START_TENTHS,
    BAUD,
    FREQ_START_HZ,
    FREQ_STEP_HZ,
    parse_capture,
    read_available,
    render_zc_figure,
    split_complete_dumps,
)

import matplotlib.pyplot as plt


def use_interactive_backend() -> str:
    """scope_common forces the Agg (headless) backend; switch to an on-screen one."""
    for backend in ("TkAgg", "QtAgg", "Qt5Agg", "MacOSX"):
        try:
            plt.switch_backend(backend)
            return backend
        except Exception:
            continue
    raise SystemExit(
        "no interactive matplotlib backend found; install tkinter (python3-tk) or PyQt"
    )


def setup_motor(ser: serial.Serial, hz: int, amp: float) -> None:
    """Reset the firmware (q -> 60 Hz / 8.0 %) then ramp to the requested point
    using the same relative keys the firmware exposes."""
    ser.reset_input_buffer()
    ser.write(b"q")
    time.sleep(0.1)
    read_available(ser, max_s=0.6)

    target_hz = int(round(hz / 10.0) * 10)
    steps_hz = (target_hz - FREQ_START_HZ) // FREQ_STEP_HZ
    freq_key = b"f" if steps_hz >= 0 else b"v"
    for _ in range(abs(steps_hz)):
        ser.write(freq_key)
        time.sleep(0.02)

    delta_tenths = int(round(amp * 10)) - AMP_START_TENTHS
    full, tenths = divmod(abs(delta_tenths), 10)
    full_key = b"a" if delta_tenths >= 0 else b"z"
    tenth_key = b"+" if delta_tenths >= 0 else b"-"
    for _ in range(full):
        ser.write(full_key)
        time.sleep(0.02)
    for _ in range(tenths):
        ser.write(tenth_key)
        time.sleep(0.02)
    read_available(ser, max_s=0.4)


def parse_args() -> argparse.Namespace:
    p = argparse.ArgumentParser(description="Live streaming ZC viewer for rinz scope1")
    p.add_argument("port", help="UART port, e.g. COM41")
    p.add_argument("--baud", type=int, default=BAUD, help=f"UART baud (default {BAUD})")
    p.add_argument("--hz", type=float, default=120, help="electrical frequency (default 120)")
    p.add_argument("--amp", type=float, default=15.0, help="amplitude %% (default 15)")
    p.add_argument("--refresh", type=float, default=1.0, help="seconds between redraws (default 1.0)")
    p.add_argument("--zc-window", type=int, default=3, help="ZC smoothing window (default 3)")
    p.add_argument("--no-init", action="store_true", help="do not reset/ramp the motor before streaming")
    return p.parse_args()


def main() -> int:
    args = parse_args()
    backend = use_interactive_backend()

    with serial.Serial(args.port, args.baud, timeout=0.02) as ser:
        ser.reset_input_buffer()
        ser.reset_output_buffer()
        if not args.no_init:
            print(f"setup: hz={args.hz:g} amp={args.amp:g}% (matplotlib backend {backend})")
            setup_motor(ser, int(round(args.hz)), args.amp)

        fig = plt.figure(figsize=(14, 9))
        try:
            fig.canvas.manager.set_window_title("rinz live ZC stream")
        except Exception:
            pass

        running = {"on": True}
        fig.canvas.mpl_connect("close_event", lambda _evt: running.__setitem__("on", False))

        # Forward control keys from the (focused) plot window to the firmware. The
        # firmware applies them between dumps without stopping the stream.
        # f/v = freq +/-10Hz, a/z = amp +/-1%, +/- (or =/-) = amp +/-0.1%, q = reset.
        keymap = {
            "f": b"f", "v": b"v", "a": b"a", "z": b"z",
            "+": b"+", "=": b"+", "-": b"-", "q": b"q",
        }

        def on_key(event):
            if event.key in ("escape",):
                running["on"] = False
                return
            byte = keymap.get(event.key)
            if byte is not None:
                ser.write(byte)
                ser.flush()

        # Drop matplotlib's own single-key shortcuts (f=fullscreen, q=quit, s=save,
        # a, etc.) so our handler fully owns the keyboard.
        for rc in list(plt.rcParams):
            if rc.startswith("keymap."):
                plt.rcParams[rc] = []
        fig.canvas.mpl_connect("key_press_event", on_key)
        plt.show(block=False)

        # Kick off the firmware continuous stream.
        ser.write(b"l")
        ser.flush()
        print(
            "streaming. Focus the PLOT WINDOW, then:\n"
            "  f/v = freq +/-10Hz   a/z = amp +/-1%   +/- = amp +/-0.1%   q = reset\n"
            "  esc or close window = quit (motor killed)"
        )

        buffer = ""
        latest = None
        dumps = 0
        last_render = 0.0
        try:
            while running["on"]:
                n = ser.in_waiting
                chunk = ser.read(n) if n else ser.read(1)
                if chunk:
                    buffer += chunk.decode("ascii", errors="replace")
                    segments, buffer = split_complete_dumps(buffer)
                    for seg in segments:
                        try:
                            cap = parse_capture(seg)
                            cap.debug.setdefault("mode", "six-step")
                            latest = cap
                            dumps += 1
                        except Exception:
                            pass
                    # Guard against unbounded growth if 'end' never arrives.
                    if len(buffer) > 1_000_000:
                        buffer = buffer[-100_000:]

                now = time.time()
                if latest is not None and now - last_render >= args.refresh:
                    try:
                        render_zc_figure(latest, fig, smooth_window=args.zc_window)
                        fig.canvas.draw_idle()
                    except Exception as exc:
                        print(f"\nrender error: {exc}")
                    last_render = now
                    print(
                        f"\rdumps={dumps} frames={latest.frames} "
                        f"hz={latest.debug.get('hz', '?')} amp={latest.debug.get('amp', '?')}   ",
                        end="",
                        flush=True,
                    )

                plt.pause(0.01)
        except KeyboardInterrupt:
            pass
        finally:
            # Any key stops the stream; 'w' also kills the motor on the way out.
            try:
                ser.write(b"w")
                ser.flush()
            except Exception:
                pass
            print("\nstream stopped; motor killed")
    return 0


if __name__ == "__main__":
    sys.exit(main())

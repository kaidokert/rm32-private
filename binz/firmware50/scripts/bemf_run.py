#!/usr/bin/env python3
"""Drive firmware50's closed-loop BEMF segment and capture each run to a file.

One run per invocation of the `b` command, captured verbatim under
`captures/<date>/`, with the ELF SHA-256 recorded alongside so a capture can
never be separated from the image that produced it.

Every exit path sends the stop key, including exceptions and Ctrl-C: an
open-loop or desynced drive on this bench is a heater, so the bridge is never
left energised because a host script died.

Usage:
    python scripts/bemf_run.py --runs 3 --label qual
    python scripts/bemf_run.py --runs 1 --label probe --timeout 90
"""

from __future__ import annotations

import argparse
import datetime
import hashlib
import zlib
import pathlib
import subprocess
import sys
import json
import time

try:
    import serial  # type: ignore
except ImportError:
    sys.exit("pyserial required: pip install pyserial")

# The console is USART3 on PC10/PC11, bridged by the FTDI USB-TTL adapter on
# COM41 -- NOT the ST-Link VCP on COM7. binz's "VCP = COM7" note belongs to its
# older EVLDRIVE102H rig. Getting this wrong costs more than a retry: COM7
# opens cleanly and then returns nothing, which looks exactly like a dead board,
# and chasing it as a probe fault is what wedged SWD (notebook E018).
DEFAULT_PORT = "COM41"

REPO = pathlib.Path(__file__).resolve().parent.parent
ELF = REPO / "target" / "thumbv6m-none-eabi" / "release" / "shell-pwm"

# The stop keys the firmware's parser treats as unconditional.
STOP = b"o"

# Lines worth echoing to the operator's terminal; everything is still written
# to the capture file.
SUMMARY_PREFIXES = (
    "BEMFRUN",
    "PREFLIGHT",
    "BEMFDONE",
    "BEMFGATE",
    "BEMFRATE",
    "BEMFSECTOR",
    "BEMFPHASE",
    "BEMFREVISIT",
    "BEMFCATCH",
    "BEMFDRIVEN",
    "BEMFDRVROWS",
    "BEMFREF",
    "BEMFRCOMP",
    "BEMFGUARD",
    "BEMFINJECT",
    "POSTSTOP",
    "PREFLIGHT",
    "BEMFCURRENT",
    "BEMFWITNESS",
    "BEMFNODES",
    "BEMFCOAST",
    "SINEPROBE",
    "RESETCAUSE",
    "SINEPROBECOAST",
    "COASTTIMING",
    "NEUTRAL",
    "BEMFRESTARTRUN",
    "BEMFRESTART",
    "ZEROSETTLE",
)

# A run is over once the coast timing -- the last line of both the `b` run and
# the `k` sine probe -- has been printed. The restart campaign (`R`) prints two
# segments and ends on its own summary line.
END_MARKER = "COASTTIMING"
END_MARKERS = {"R": "BEMFRESTART ", "Z": "BEMFRESTART ", "z": "ZEROSETTLEDONE"}


# The reference's figures per duty (tenths): electrical speed and the
# signed-residual current proxy, from DUTY_50_CAMPAIGN.md's rung table. That
# table is the bench-rate-curve diagnostic image 10CD4D47..., one run per rung
# (notebook E083/E084); the frozen oracle prints no current census.
ORACLE = {100: (401, 45), 150: (704, 58), 200: (941, 167), 250: (1186, 326),
          275: (1279, 423), 288: (1327, 473), 300: (1371, 519), 325: (1468, 630),
          338: (1519, 691), 350: (1564, 740), 375: (1650, 843), 400: (1736, 945),
          425: (1815, 1091), 450: (1893, 1237), 475: (1995, 1420),
          500: (2096, 1603)}  # see cohort.ORACLE_INTERPOLATED

# Rung keys -> target duty (tenths). `b` runs the image's default, 15%.
# Campaign 6 (E137): uppercase qualifies a rung, lowercase explores it.
RUNG = {"b": 150, "2": 200, "5": 250,
        "A": 275, "Y": 288, "C": 300, "D": 325, "M": 338, "E": 350, "J": 375}
EXPLORE = {"a": 275, "y": 288, "c": 300, "d": 325, "m": 338, "e": 350, "j": 375}
# An exploratory run is driven on a short window: every gate but the dwell.
EXPLORE_HOLD_MS = 9_000


def _fields(line: str) -> dict[str, str]:
    out = {}
    for tok in line.split()[1:]:
        if "=" in tok:
            k, v = tok.split("=", 1)
            out[k] = v
    return out


def reference_line(capture: pathlib.Path) -> str | None:
    """Put the run's speed and current beside the oracle's at the same duty.

    Speed is from the rotor, not the switch count: the bridge-off coast
    spacing, one full electrical period = two consecutive transitions on the
    watched phase. A mismatch above 20% on either is a stop (goal item 4)."""
    rate = cur = coast = run = None
    for line in capture.read_text(encoding="utf-8").splitlines():
        if line.startswith("BEMFRATE"):
            rate = _fields(line)
        elif line.startswith("BEMFCURRENT"):
            cur = _fields(line)
        elif line.startswith("COASTTIMING"):
            coast = _fields(line)
        elif line.startswith("BEMFRUN"):
            run = _fields(line)
    if not (rate and cur and coast and run):
        return None
    duty = int(run.get("target_duty_tenths", 0))
    if duty not in ORACLE:
        return None
    ref_ehz, ref_ma = ORACLE[duty]
    iv = [int(x) for x in coast.get("iv_us", "").split(",") if x]
    import cohort  # noqa: PLC0415 - one coast estimator for every judgement (E113)

    coast_ehz = cohort.coast_ehz(iv)
    hold_ma = int(cur.get("hold_ma", 0))
    loop_ehz = int(rate.get("ehz_from_sector", 0))
    speed_pct = 100.0 * (coast_ehz - ref_ehz) / ref_ehz
    cur_pct = 100.0 * (hold_ma - ref_ma) / ref_ma
    stop = abs(speed_pct) > 20.0 or abs(cur_pct) > 20.0
    # The accepted-crossing rate against the rotor's own speed (six crossings
    # per electrical cycle), independent of the loop's own interval estimate.
    zc = int(rate.get("zc_per_s", 0))
    zc_rotor = (1000 * zc // (6 * coast_ehz)) if coast_ehz else 0
    return (
        f"BEMFREF duty_tenths={duty} coast_ehz={coast_ehz} loop_ehz={loop_ehz} "
        f"oracle_ehz={ref_ehz} speed_pct={speed_pct:+.1f} speed_within_5pct={int(abs(speed_pct) <= 5.0)} "
        f"hold_ma={hold_ma} oracle_ma={ref_ma} current_pct={cur_pct:+.1f} "
        # The circular metric is not printed as the run's rate any more: it
        # measures truncation (E172). The gate's own quantity is computed by
        # `cohort.parse` and printed by the ladder summary instead.
        f"zc_rate_circular_ignored={rate.get('zc_rate_permille_of_expected', '?')} "
        f"zc_permille_of_6x_coast={zc_rotor} "
        f"hold_ms={rate.get('hold_ms', '?')} verdict={'STOP' if stop else 'ok'}"
    )


# The ladder the fixture enforces (goal item 9, notebook E109): a rung may be
# launched only when the rung below it has a passing report on this same ELF.
# A rung's report is its last three runs on that ELF: every run passes gates
# 1-3 (cohort.run_gates) and the rung's means pass the oracle comparison
# (cohort.rung_oracle). `R` (restart at 25%) needs the 25% rung.
LADDER_PREREQ = {"2": 150, "5": 200, "R": 250,
                 "a": 250, "A": 250, "y": 275, "Y": 275, "c": 275, "C": 275,
                 "d": 300, "D": 300, "m": 325, "M": 325, "e": 338, "E": 338,
                 "j": 350, "J": 350, "Z": 475}
LADDER_FILE = REPO / "captures" / "ladder_state.json"
RUNG_RUNS = 3


def _ladder_load() -> dict:
    try:
        return json.loads(LADDER_FILE.read_text(encoding="utf-8"))
    except (OSError, ValueError):
        return {}


def _ladder_save(state: dict) -> None:
    LADDER_FILE.write_text(json.dumps(state, indent=1), encoding="utf-8")


def rung_report(state: dict, sha: str, duty: int) -> tuple[bool, list[str]]:
    """Pass/fail of the rung at `duty` on this ELF, with its reasons."""
    import cohort  # noqa: PLC0415 - sibling script

    runs = state.get(sha, {}).get(str(duty), [])[-RUNG_RUNS:]
    if len(runs) < RUNG_RUNS:
        return False, [f"only {len(runs)} run(s) on this ELF at {duty / 10:.0f}%"]
    fails = [f"{r['file']}: {', '.join(r['fails'])}" for r in runs if r["fails"]]
    fails += cohort.rung_oracle(runs)
    return not fails, fails


def ladder_admit(command: str, sha: str, rung_duty: int = 0) -> tuple[bool, str]:
    if command in ("l", "L"):
        if rung_duty not in ORACLE:
            return False, f"--rung-duty {rung_duty} is not a known rung"
        # 40% follows 37.5%, then every 2.5% step follows the one below it.
        need = 375 if rung_duty == 400 else rung_duty - 25
    else:
        need = LADDER_PREREQ.get(command)
    if need is None:
        return True, ""
    ok, why = rung_report(_ladder_load(), sha, need)
    return ok, "; ".join(why)


def ladder_record(capture: pathlib.Path, sha: str, explore: bool = False, expect_duty: int = 0) -> list[str] | None:
    """Judge one completed rung run (gates 1-3) and record it. Returns the
    failures, or None when the capture is not a rung run."""
    import cohort  # noqa: PLC0415

    r = cohort.parse(capture)
    if r is None or r["duty"] not in (150, 200, 250, 275, 288, 300, 325, 338, 350, 375, 400, 425, 450, 475, 500):
        return None
    # `l`/`L` take their duty from shell state, so the caller says which rung
    # it believes it is running and the capture has to agree. Without this the
    # ladder can admit a rung on the strength of the one below a *different*
    # rung -- which is exactly what happened on the first climb run (E148).
    if expect_duty and r["duty"] != expect_duty:
        return [f"capture ran {r['duty']} tenths, not the {expect_duty} asked for: shell duty not where it was assumed"]
    if explore:
        # Never recorded as a rung run: an exploratory run cannot meet the
        # dwell and must not count toward a 3/3 cohort.
        return cohort.run_gates(r, EXPLORE_HOLD_MS)
    r["fails"] = cohort.run_gates(r)
    state = _ladder_load()
    state.setdefault(sha, {}).setdefault(str(r["duty"]), []).append(r)
    _ladder_save(state)
    return r["fails"]


def restart_verdict(capture: pathlib.Path) -> list[str]:
    """Goal item 5 for one `R` capture, judged in the script."""
    line = next(
        (l for l in capture.read_text(encoding="utf-8").splitlines() if l.startswith("BEMFRESTART ")),
        "",
    )
    f = dict(t.split("=", 1) for t in line.split()[1:] if "=" in t)
    fails = []
    if f.get("first_reason") != "8":
        fails.append(f"first_reason {f.get('first_reason')} != 8")
    if f.get("recovered") != "1":
        fails.append("not recovered")
    if f.get("drive_end_ms") != f.get("window_ms"):
        fails.append(f"drive end {f.get('drive_end_ms')} != window {f.get('window_ms')}")
    return fails


def elf_sha256() -> str:
    if not ELF.exists():
        return "no-elf"
    return hashlib.sha256(ELF.read_bytes()).hexdigest().upper()


def elf_crc32() -> str:
    """The CRC32 the notebook and `captures/elf/` name images by.

    The header carried only the SHA-256 while every notebook entry, every gate
    file and every archived filename used the CRC32, so tying a capture to its
    image meant joining two files by hand -- and a reviewer could not confirm
    which image produced a given dump at all (E186 SS4). Both go in now.
    """
    if not ELF.exists():
        return "no-elf"
    return f"{zlib.crc32(ELF.read_bytes()) & 0xFFFFFFFF:08X}"


def capture_one(
    port: serial.Serial,
    out: pathlib.Path,
    timeout_s: float,
    command: bytes,
    abort_after: float = 0.0,
    end_marker: str = END_MARKER,
) -> bool:
    """Send the run command, stream until the coast timing line or timeout.
    True if it completed."""
    port.reset_input_buffer()
    port.write(command)
    port.flush()

    started = time.monotonic()
    deadline = started + timeout_s
    abort_sent = abort_after <= 0
    completed = False
    with out.open("w", encoding="utf-8", newline="\n") as fh:
        fh.write(f"# elf_crc32 {elf_crc32()}\n")
        fh.write(f"# elf_sha256 {elf_sha256()}\n")
        fh.write(f"# started {datetime.datetime.now().isoformat(timespec='seconds')}\n")
        pending = b""
        while time.monotonic() < deadline:
            if not abort_sent and time.monotonic() - started >= abort_after:
                port.write(STOP)
                port.flush()
                abort_sent = True
                fh.write(f"# host abort byte sent at +{abort_after:.1f} s\n")
            chunk = port.read(4096)
            if not chunk:
                continue
            pending += chunk
            while b"\n" in pending:
                raw, pending = pending.split(b"\n", 1)
                line = raw.decode("utf-8", "replace").strip()
                if not line:
                    continue
                fh.write(line + "\n")
                fh.flush()
                if line.startswith(SUMMARY_PREFIXES):
                    print("   " + line, flush=True)
                if line.startswith(end_marker):
                    completed = True
            if completed:
                break
    return completed


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--port", default=DEFAULT_PORT)
    ap.add_argument("--baud", type=int, default=115200)
    ap.add_argument(
        "--elf",
        default="",
        help="the image to run and to record in the capture header; with "
        "--flash it is also programmed. Without this the header records "
        "whatever is at the default build path, which is NOT necessarily what "
        "is on the chip -- a bisect against an archived image mislabelled its "
        "own captures that way (E164)",
    )
    ap.add_argument(
        "--flash",
        action="store_true",
        help="program --elf (probe-rs download + reset) before the runs, so the "
        "recorded hash is necessarily the image that ran",
    )
    ap.add_argument("--runs", type=int, default=1)
    ap.add_argument("--label", default="run")
    ap.add_argument(
        "--timeout",
        type=float,
        default=75.0,
        help="per-run ceiling in seconds; the segment itself is 40 s plus coast",
    )
    ap.add_argument("--settle", type=float, default=3.0, help="rest between runs")
    ap.add_argument(
        "--pre",
        default="",
        help="keys to send once before the runs and not record, e.g. '+' to step "
        "the shell's climb duty up one rung. `l`/`L` drive whatever the shell "
        "holds, so without this a climb run repeats the rung it is already at -- "
        "which is what three '42.5%' runs actually did at 40% (E185). "
        "`sag_run.py` has had this; this runner had not.",
    )
    ap.add_argument(
        "--rung-duty",
        type=int,
        default=0,
        help="the duty (tenths) `l`/`L` are set to; required for them, since "
        "the shell holds the climb duty and the key alone does not say it. "
        "Every capture is checked against this value.",
    )
    ap.add_argument(
        "--command",
        default="b",
        choices=["b", "2", "5", "R", "z", "k", "T", "G", "F", "N", "U", "H", "V", "I", "W",
                 "t", "g", "f", "n", "u", "h", "v", "i", "w",
                 "8", "3", "4", "6", "7", "9",
                 "a", "c", "d", "e", "j", "y", "m", "x", "Z", "l", "L", "+", "-",
                 "A", "C", "D", "E", "J", "Y", "M"],
        help="b = full BEMF run at 15%%; 2 / 5 = the 20%% / 25%% rungs; "
        "8/3/4/6/7/9 = short ~15 s holds at 25/23/21/19/17/15%% (metered-current points, not rungs); R = normal-start recovery at 25%%; k = sine probe; T/G/F/N/U/V/I = gate-4 provocations "
        "(tracking, tick gap, feedback stale, nFAULT, comparator storm, bus sag, "
        "average current), each 3 s into the 15%% closed loop; lowercase = the same at 25%%, 2 s after the ramp reaches target",
    )
    ap.add_argument(
        "--abort-after",
        type=float,
        default=0.0,
        help="gate-4 HostAbort provocation: send the stop byte this many seconds "
        "after the run command (0 = never)",
    )
    ap.add_argument(
        "--step-check",
        action="store_true",
        help="refactor step gate (goal item 8): one unjudged 15%% warm-up run, then "
        "exactly one 25%% run judged against the E101-E108 cohort spread and gates "
        "1-3; not a rung, records nothing. The warm-up matches the cohort's "
        "conditions: the current proxy's zero follows the driver's temperature "
        "(E092/E093), and a cold first run read 291 mA against the cohort's "
        "331-372 while the same image warm read 338 (E110)",
    )
    args = ap.parse_args()
    global ELF  # noqa: PLW0603
    if args.elf:
        ELF = pathlib.Path(args.elf)
        if not ELF.exists():
            print(f"no such image: {ELF}")
            return 2
    if args.flash:
        probe = "0483:374b:066CFF343433464757233430"
        for cmd in (["probe-rs", "download", "--chip", "STM32G071RBTx", "--probe", probe, str(ELF)],
                    ["probe-rs", "reset", "--chip", "STM32G071RBTx", "--probe", probe]):
            if subprocess.run(cmd, check=False).returncode:
                print("flash failed; refusing to run")
                return 2
        time.sleep(1.0)
    if args.step_check:
        args.command, args.runs = "5", 1

    day = datetime.date.today().isoformat()
    outdir = REPO / "captures" / day
    outdir.mkdir(parents=True, exist_ok=True)

    sha = elf_sha256()
    print(f"elf  {sha}")
    print(f"out  {outdir}")

    admitted, why = (True, "") if args.step_check else ladder_admit(args.command, sha, args.rung_duty)
    if not admitted:
        # `l`/`L` take their prerequisite from `--rung-duty`, not from
        # `LADDER_PREREQ`, so the old message raised `KeyError: 'L'` -- the
        # refusal path itself crashed, and it is the path a new image always
        # takes (E188 SS5).
        if args.command in ("l", "L"):
            need = 375 if args.rung_duty == 400 else args.rung_duty - 25
        else:
            need = LADDER_PREREQ.get(args.command)
        where = f"the {need / 10:.1f}% rung" if need else "its prerequisite"
        print(f"LADDER REFUSED: command {args.command} needs {where} passed on this ELF: {why}")
        print("  An exploratory cohort that is not claiming a rung passes --step-check,")
        print("  which judges the run but records it against no rung.")
        return 2
    failed_runs = 0

    port = serial.Serial(args.port, args.baud, timeout=0.2)
    ok = 0
    try:
        # Clear anything the firmware queued before we attached.
        time.sleep(0.3)
        port.reset_input_buffer()
        # **A colliding label is refused, not overwritten** (E186's
        # label-collision item). Re-running a label destroyed nine good
        # captures during this campaign's ladder climb -- three 15%, three 20%
        # and three 25% runs -- and the campaign's own rule is that every
        # capture is retained. The ladder still holds their summaries, but the
        # captures themselves are gone.
        clash = [
            outdir / f"{args.label}_{i:02d}.txt"
            for i in range(1, args.runs + 1)
            if (outdir / f"{args.label}_{i:02d}.txt").exists()
        ]
        if clash:
            print("REFUSED: these captures already exist; choose another --label:")
            for c in clash:
                print(f"  {c}")
            return 2
        for key in args.pre:
            print(f"== pre-key {key!r} (not recorded)")
            port.write(key.encode())
            port.flush()
            time.sleep(0.4)
            while True:
                line = port.readline()
                if not line:
                    break
                print("   " + line.decode(errors="replace").strip())
        for i in range(1, args.runs + 1):
            out = outdir / f"{args.label}_{i:02d}.txt"
            if args.step_check and i == 1:
                warm = outdir / f"{args.label}_warmup.txt"
                print(f"\n== warm-up (15%, not judged) -> {warm.name}")
                capture_one(port, warm, args.timeout, b"b", 0.0, END_MARKER)
                port.write(STOP)
                port.flush()
                time.sleep(max(args.settle, 20.0))
            print(f"\n== run {i}/{args.runs} -> {out.name}")
            if capture_one(
                port,
                out,
                args.timeout,
                args.command.encode("ascii"),
                args.abort_after,
                END_MARKERS.get(args.command, END_MARKER),
            ):
                ok += 1
                ref = reference_line(out)
                if ref:
                    with out.open("a", encoding="utf-8", newline="\n") as fh:
                        fh.write(ref + "\n")
                    print("   " + ref, flush=True)
                if args.step_check:
                    import cohort  # noqa: PLC0415

                    fails = cohort.check(out)
                    if fails:
                        failed_runs += 1
                    print("   STEP CHECK " + ("FAIL: " + "; ".join(fails) if fails else "PASS"), flush=True)
                elif (args.command in RUNG or args.command in EXPLORE or args.command in ("l", "L")) and args.abort_after <= 0:
                    # A host-abort provocation is not a rung run (E124). An
                    # exploratory run (E137) is judged but never recorded.
                    explore = args.command in EXPLORE or args.command == "l"
                    fails = ladder_record(out, sha, explore, args.rung_duty)
                    label = "EXPLORE" if explore else "RUN"
                    if fails:
                        failed_runs += 1
                        print(f"   {label} FAIL: " + "; ".join(fails), flush=True)
                    elif fails is not None:
                        print(f"   {label} PASS (gates 1-3)", flush=True)
                elif args.command in ("R", "Z"):
                    fails = restart_verdict(out)
                    if fails:
                        failed_runs += 1
                    print("   RESTART " + ("FAIL: " + "; ".join(fails) if fails else "PASS"), flush=True)
            else:
                print(f"   !! run did not reach its end marker within the timeout")
            # Stop and rest between runs so each starts from the same state.
            port.write(STOP)
            port.flush()
            if i < args.runs:
                time.sleep(args.settle)
    except KeyboardInterrupt:
        print("\ninterrupted", flush=True)
    finally:
        # Unconditional safe-off, then leave the port clean.
        try:
            port.write(STOP)
            port.flush()
            time.sleep(0.2)
        finally:
            port.close()

    print(f"\n{ok}/{args.runs} runs completed, {failed_runs} failed their gates")
    rung = None if args.step_check else RUNG.get(args.command)
    if args.command in ("l", "L"):
        rung = args.rung_duty if args.command == "L" else None
    if rung:
        passed, why = rung_report(_ladder_load(), sha, rung)
        print(f"RUNG {rung / 10:.0f}%: {'PASS' if passed else 'NOT PASSED'}"
              + ("" if passed else f" ({'; '.join(why)})"))
        import cohort  # noqa: PLC0415

        runs = _ladder_load().get(sha, {}).get(str(rung), [])[-RUNG_RUNS:]
        print("   " + cohort.rung_current_note(runs))
    return 0 if ok == args.runs and failed_runs == 0 else 1


if __name__ == "__main__":
    raise SystemExit(main())

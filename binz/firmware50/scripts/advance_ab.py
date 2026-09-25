"""The advance 22-vs-20 A/B at one rung, ABAB, one session, flashed per side.

The decision this changes (E302 rule 3): which advance schedule the 550 -> 600
climb uses, and therefore which image goes forward to full qualification. It is
six runs, against the forty-eight a ladder would have cost on an image the A/B
might reject.

The hypothesis. `thin_count` counts commutations where `wait - spent <= 2 us`.
It is 0 through rung 350 and then 8 / 11 / 121 / 1062 / 4074 per 1e6 accepted at
rungs 425 / 450 / 475 / 500 / 525, while `spent_max_us` stays pinned at 11 and
`late_arms` stays 0. By the real integer `wait_time` (`src/commutation.rs`),
advance 22 clears an 11 us arm only above ci = 72 and advance 20 above ci = 60,
so advance 20 should buy ~12 us of interval headroom.

The predeclared bar (E301/E302): **advance 20 must cut `thin_count` at rung 500
from ~700 per run to under 50**, with `late_arms = 0` on both sides and the rate
identity inside 992..1008 on both. If it does not cut `thin_count` by at least
5x, the thin condition is not about `wait` and the arm-path model is refuted.

The known methodological weakness, stated here rather than discovered later:
less advance means less torque, so the rotor may run slower, so `ci` gets
LONGER -- which relieves `thin_count` by itself. A `thin_count` drop therefore
does not cleanly evidence "more wait margin" unless the speed is held in view.
So this script reports `ehz_from_sector` and `hold_ma` per side, and the
disposition must read `thin_count` **together with** the speed, not alone.

Per [[feedback-microsecond-ab-needs-three-runs]]: three runs a side, alternated
within one session, each side flashed and hashed at its own turn.
"""
from __future__ import annotations

import argparse
import hashlib
import pathlib
import re
import statistics
import subprocess
import sys
import time

REPO = pathlib.Path(__file__).resolve().parent.parent
PROBE = "0483:374b:066CFF343433464757233430"
CHIP = "STM32G071RBTx"

SIDES = {
    "A22": REPO / "captures/elf/5D4BF25C.e301-fixed-adv22.elf",
    "B20": REPO / "captures/elf/8FE909B3.e301-fixed-adv20.elf",
}

# Absolute duty for the `L` rung command: 11 down to the floor, then N up. The
# shell's climb duty is relative state, so it must be driven absolutely --
# a relative step once made three "42.5%" runs drive 40%
# ([[feedback-guard-relative-shell-state]]).
PRE_FOR_RUNG = {400: "+", 425: "++", 450: "+++", 475: "++++", 500: "+++++", 525: "++++++"}

ANCHOR_PROOF = ("isr_diff 480263F1 vs 5D4BF25C: 4 roots identical "
                "(37/728/332/155 instructions); A/B varies only ADVANCE_HIGH")

FIELDS = ("thin_count", "spent_max_us", "late_arms", "ci_us", "ci_min_us",
          "accepted", "ehz_from_sector", "hold_ma", "ceiling_tenths")


def sha(p: pathlib.Path) -> str:
    return hashlib.sha256(p.read_bytes()).hexdigest().upper()


def flash(elf: pathlib.Path) -> bool:
    for cmd in (["probe-rs", "download", "--chip", CHIP, "--probe", PROBE, str(elf)],
                ["probe-rs", "reset", "--chip", CHIP, "--probe", PROBE]):
        if subprocess.run(cmd, check=False).returncode:
            return False
    time.sleep(1.0)
    return True


def run_one(elf: pathlib.Path, rung: int, label: str, port: str) -> pathlib.Path | None:
    cmd = [
        sys.executable, str(REPO / "scripts/bemf_run.py"),
        "--elf", str(elf), "--port", port,
        "--command", "L", "--runs", "1", "--label", label,
        "--timeout", "150", "--settle", "20",
        "--rung-duty", str(rung),
        f"--pre=-----------{PRE_FOR_RUNG[rung]}",
        "--anchor", "--anchor-proof", ANCHOR_PROOF,
    ]
    print("   " + " ".join(cmd[3:]))
    subprocess.run(cmd, check=False, cwd=REPO)
    day = time.strftime("%Y-%m-%d")
    hits = sorted((REPO / "captures" / day).glob(f"{label}*.txt"))
    return hits[-1] if hits else None


def read(capture: pathlib.Path) -> dict[str, int]:
    t = capture.read_text(encoding="utf-8", errors="replace")
    out: dict[str, int] = {}
    for f in FIELDS:
        m = re.search(rf"\b{f}=(-?\d+)", t)
        if m:
            out[f] = int(m.group(1))
    m = re.search(r"BEMFDONE .*?\breason=(\d+)", t)
    if m:
        out["reason"] = int(m.group(1))
    sys.path.insert(0, str(REPO / "scripts"))
    import cohort  # noqa: PLC0415
    d = cohort.parse(capture)
    if d:
        out["rvc"] = d["rate_vs_coast_permille"]
    return out


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--rung", type=int, default=500)
    ap.add_argument("--rounds", type=int, default=3)
    ap.add_argument("--port", default="COM41")
    ap.add_argument("--label", default="e303-ab")
    args = ap.parse_args()

    for name, elf in SIDES.items():
        if not elf.exists():
            print(f"missing image for side {name}: {elf}")
            return 2
        print(f"side {name}: {elf.name}  sha256 {sha(elf)[:16]}...")
    if args.rung not in PRE_FOR_RUNG:
        print(f"no absolute-duty recipe for rung {args.rung}")
        return 2

    results: dict[str, list[dict[str, int]]] = {k: [] for k in SIDES}
    for rnd in range(1, args.rounds + 1):
        # Alternate WITHIN the session, flashing at each turn, so a drift in
        # bench state falls on both sides equally instead of on whichever ran
        # second.
        for name, elf in SIDES.items():
            print(f"\n=== round {rnd}/{args.rounds}  side {name}  rung {args.rung}")
            if not flash(elf):
                print("flash failed; stopping rather than running the wrong image")
                return 2
            cap = run_one(elf, args.rung, f"{args.label}-{name}_{rnd}", args.port)
            if cap is None:
                print("no capture written; stopping")
                return 2
            row = read(cap)
            results[name].append(row)
            print(f"   -> thin={row.get('thin_count')} late={row.get('late_arms')} "
                  f"ci={row.get('ci_us')} ehz={row.get('ehz_from_sector')} "
                  f"ma={row.get('hold_ma')} rvc={row.get('rvc')} "
                  f"reason={row.get('reason')}")

    print(f"\n{'=' * 72}\nA/B at rung {args.rung}, {args.rounds} runs a side, ABAB\n")
    hdr = ("side", "thin", "late", "ci", "ci_min", "ehz", "hold_ma", "rvc", "reason")
    print("".join(f"{h:>10}" for h in hdr))
    means: dict[str, dict[str, float]] = {}
    for name, rows in results.items():
        if not rows:
            continue
        def mean(k: str) -> float:
            v = [r[k] for r in rows if k in r]
            return statistics.mean(v) if v else float("nan")
        means[name] = {k: mean(k) for k in
                       ("thin_count", "late_arms", "ci_us", "ci_min_us",
                        "ehz_from_sector", "hold_ma", "rvc")}
        m = means[name]
        print(f"{name:>10}{m['thin_count']:>10.0f}{m['late_arms']:>10.0f}"
              f"{m['ci_us']:>10.0f}{m['ci_min_us']:>10.0f}"
              f"{m['ehz_from_sector']:>10.0f}{m['hold_ma']:>10.0f}"
              f"{m['rvc']:>10.0f}"
              f"{str(sorted({r.get('reason') for r in rows})):>10}")
        print("".join(f"{'':>10}" for _ in hdr) + f"  runs: {[r.get('thin_count') for r in rows]}")

    if len(means) == 2:
        a, b = means["A22"], means["B20"]
        print(f"\nPREDECLARED BAR (E301/E302): advance 20 must cut thin_count to <50")
        print(f"  A22 thin = {a['thin_count']:.0f}   B20 thin = {b['thin_count']:.0f}")
        ratio = a["thin_count"] / b["thin_count"] if b["thin_count"] else float("inf")
        print(f"  ratio = {ratio:.1f}x  (bar: >= 5x AND B20 < 50)")
        met = b["thin_count"] < 50 and ratio >= 5
        print(f"  => {'BAR MET' if met else 'BAR NOT MET -- arm-path model refuted'}")
        # The confound this A/B cannot remove by itself.
        dehz = 100 * (b["ehz_from_sector"] - a["ehz_from_sector"]) / a["ehz_from_sector"]
        dci = 100 * (b["ci_us"] - a["ci_us"]) / a["ci_us"]
        print(f"\n  CONFOUND CHECK: advance 20 changes torque, so the rotor speed "
              f"and\n  therefore ci move too -- and ci is what thin_count depends on.")
        print(f"    speed {dehz:+.1f}%   ci {dci:+.1f}%")
        print(f"  If ci rose materially, part of the thin_count drop is the longer "
              f"interval,\n  not the wider wait. Read the two together.")
        if any(m["late_arms"] for m in means.values()):
            print("\n  !! late_arms non-zero on a side: that is the real detector firing.")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

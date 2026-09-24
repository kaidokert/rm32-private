"""Walk the qualification ladder on one ELF, stopping at the first failure.

The ladder is keyed on the ELF sha256 and each rung is admitted only when the
rung below has three passing runs on that same image, so qualification is a
chain of 19 rungs from 150 to 600 tenths. This drives that chain.

Why a driver rather than 57 hand-issued commands:

  * **E239's process failure.** Its stopping rule was breached and the batch
    ran on, because three runs were launched unattended in one invocation and
    nothing checked between them. Here every run is judged as it lands and the
    walk stops on the first failure, so a rule can actually end it.
  * **E148/E185.** `l`/`L` drive whatever duty the shell holds, so a rung is
    commanded by stepping the climb duty with `+` presses. Getting the count
    wrong silently drives the rung below and labels it as the one above. The
    press count is computed here from the rung, once.
  * The rung chain itself is easy to get wrong by hand, and admission refuses
    rather than explaining.

It does not bypass a single gate: it calls `bemf_run.py` exactly as a hand
invocation would, in ladder mode, and reads the rung report the fixture writes.
"""
import argparse
import pathlib
import subprocess
import sys

REPO = pathlib.Path(__file__).resolve().parent.parent

# The chain, in order. Each entry is (key, duty_tenths). The keyed rungs come
# first; from 400 up the rung is driven by `L` with the climb duty stepped to
# it, which is what `--rung-duty` and `--pre` express.
CHAIN = [
    ("b", 150), ("2", 200), ("5", 250),
    ("A", 275), ("Y", 288), ("C", 300), ("D", 325), ("M", 338), ("E", 350),
    ("J", 375),
    ("L", 400), ("L", 425), ("L", 450), ("L", 475), ("L", 500),
    ("L", 525), ("L", 550), ("L", 575), ("L", 600),
]

# `climb_tenths` starts here and `+` steps it by this much (src/run/mod.rs).
CLIMB_START = 400
CLIMB_STEP = 25


def presses(duty: int) -> str:
    """The `+` string that puts the shell's climb duty exactly on `duty`.

    Computed, never counted by hand: three runs labelled 42.5% once drove 40%
    because the count was assumed (E185), and `--rung-duty` cannot catch it on
    a bypassed run because the check lives in `ladder_record`.
    """
    n = (duty - CLIMB_START) // CLIMB_STEP
    if n < 0 or CLIMB_START + n * CLIMB_STEP != duty:
        sys.exit(f"duty {duty} is not reachable from {CLIMB_START} in {CLIMB_STEP} steps")
    return "+" * n


def rung_state(sha: str, duty: int) -> tuple[bool, list[str]]:
    sys.path.insert(0, str(REPO / "scripts"))
    import bemf_run  # noqa: PLC0415

    return bemf_run.rung_report(bemf_run._ladder_load(), sha, duty)


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--elf", required=True)
    ap.add_argument("--runs", type=int, default=3)
    ap.add_argument("--from-duty", type=int, default=150)
    ap.add_argument("--to-duty", type=int, default=600,
                    help="the highest rung to attempt; the walk stops after it")
    ap.add_argument("--label", default="ladder")
    ap.add_argument("--timeout", type=int, default=150)
    ap.add_argument("--settle", type=int, default=20)
    ap.add_argument("--explore", action="store_true",
                    help="drive the lowercase exploratory run before each rung")
    ap.add_argument("--dry-run", action="store_true")
    a = ap.parse_args()

    sys.path.insert(0, str(REPO / "scripts"))
    import bemf_run  # noqa: PLC0415

    # The fixture keys ladder state by UPPERCASE sha256 (`bemf_run.elf_sha256`
    # returns `.upper()`), and its own helper reads a module global rather than
    # taking a path. Matching the case exactly matters: a lower-case key looks
    # up nothing and every rung would read as unqualified.
    import hashlib  # noqa: PLC0415

    sha = hashlib.sha256(pathlib.Path(a.elf).read_bytes()).hexdigest().upper()

    todo = [(k, d) for k, d in CHAIN if a.from_duty <= d <= a.to_duty]
    print(f"ELF   {a.elf}")
    print(f"sha   {sha[:16]}...")
    print(f"chain {len(todo)} rung(s): {[d for _, d in todo]}")

    first = True
    for key, duty in todo:
        ok, why = rung_state(sha, duty)
        if ok:
            print(f"\n=== {duty} tenths: already 3/3 on this ELF, skipping")
            continue
        cmd = [sys.executable, str(REPO / "scripts" / "bemf_run.py"),
               "--elf", a.elf, "--command", key, "--runs", str(a.runs),
               "--label", f"{a.label}-{duty}", "--timeout", str(a.timeout),
               "--settle", str(a.settle)]
        if key in ("l", "L"):
            cmd += ["--rung-duty", str(duty), f"--pre={presses(duty)}"]
        if first:
            cmd.append("--flash")   # program once; every later run is the same image
            first = False
        print(f"\n=== {duty} tenths via {key!r}"
              + (f" with {len(presses(duty))} '+' press(es)" if key in ("l", "L") else ""))
        print("    " + " ".join(cmd[1:]))
        if a.dry_run:
            continue
        r = subprocess.run(cmd, cwd=REPO)
        ok, why = rung_state(sha, duty)
        print(f"    rung {duty}: {'PASS 3/3' if ok else 'NOT PASSED -- ' + '; '.join(why)}")
        if not ok:
            print(f"\nSTOPPING at {duty} tenths. Runner exit {r.returncode}.")
            print("Every run is retained; the failure is the result, not a retry.")
            return 1
    print("\nladder walk complete for the requested range")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

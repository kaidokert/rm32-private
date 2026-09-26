"""Fixture/firmware key parity: every key the host can send must be implemented.

Why (E356). `scripts/bemf_run.py` offered `--command z`, and `END_MARKERS`
waited for a `ZEROSETTLEDONE` line the firmware emits **nowhere** -- grep `src/`
and `bin/` and it does not exist. So that choice could only ever wait out its
timeout. It also offered `k`, which the shell dispatcher did not implement at
all, and which E355 has now given a meaning (`LateArm` at 25 %) -- so the same
byte silently went from no-op to provocation with nothing checking.

Nothing tied the host's key list to `run::Controller::command`'s match, which the
notebook had already flagged as a hazard for the recorder images' `arms_a_run`.
This ties it, in the one direction that matters: **a key the fixture can send
must be a key the firmware dispatches.** The converse is fine -- the firmware may
implement keys the fixture has no reason to drive.

    python scripts/key_parity_check.py
"""
from __future__ import annotations

import pathlib
import re
import sys

ROOT = pathlib.Path(__file__).resolve().parent.parent
DISPATCHER = ROOT / "src" / "run" / "mod.rs"
FIXTURE = ROOT / "scripts" / "bemf_run.py"


def firmware_keys() -> set[str]:
    """Bytes `Controller::command` dispatches, plus the derived lowercase set."""
    src = DISPATCHER.read_text(encoding="utf-8")
    start = src.index("pub fn command<IO: Hal + Sink>")
    # The dispatcher ends at the next top-level `pub` item in the impl.
    end = src.index("pub const fn drives_a_run", start)
    body = src[start:end]
    keys = set(re.findall(r"b'(.)'", body))

    # `command` reaches the lowercase provocations by upper-casing (mod.rs), and
    # `drives_a_run` mirrors it, so every injection key implies its lowercase.
    inj = src[src.index("pub const fn inject_for"):]
    inj = inj[: inj.index("\n}")]
    for k in re.findall(r"b'(.)'", inj):
        keys.add(k)
        keys.add(k.lower())
    return keys


def fixture_keys() -> set[str]:
    src = FIXTURE.read_text(encoding="utf-8")
    m = re.search(r'"--command",.*?choices=\[(.*?)\]', src, re.S)
    if not m:
        raise SystemExit("REFUSED: could not find the --command choices list; "
                         "this check would pass vacuously.")
    return set(re.findall(r'"(.)"', m.group(1)))


def main() -> int:
    fw, fx = firmware_keys(), fixture_keys()
    if not fw or not fx:
        print(f"REFUSED: extracted {len(fw)} firmware and {len(fx)} fixture "
              f"keys; one side parsed as empty, so this proves nothing.")
        return 2
    orphans = sorted(fx - fw)
    print(f"firmware dispatches {len(fw)} keys; fixture offers {len(fx)}")
    if orphans:
        print(f"FAILED: {len(orphans)} fixture key(s) the firmware does not "
              f"dispatch: {' '.join(orphans)}")
        print("  A key the device does not implement can only time out. Remove "
              "it from `choices`, or implement it.")
        return 1
    print("OK: every key the fixture can send is dispatched by the firmware")
    # Reported, not enforced: the firmware may legitimately implement more.
    extra = sorted(fw - fx)
    if extra:
        print(f"note: {len(extra)} firmware key(s) the fixture never drives: "
              f"{' '.join(extra)}")
    return 0


if __name__ == "__main__":
    sys.exit(main())

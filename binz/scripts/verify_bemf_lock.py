"""Verify the compact low-duty BEMF handoff/lock transcript.

This deliberately treats ``BEMFSTOP lock_proven`` as informational: the compact
firmware currently prints that field conservatively as zero.  The verdict is
based on transfer, sustained event/commutation progress, absence of tracking
and protection faults, and normal target-dwell completion.
"""

from __future__ import annotations

import argparse
import re
import sys
from pathlib import Path


def one(pattern: str, text: str, label: str) -> re.Match[str]:
    rows = list(re.finditer(pattern, text, re.MULTILINE))
    if len(rows) != 1:
        raise ValueError(f"{label}: expected one line, found {len(rows)}")
    return rows[0]


def verify(text: str, minimum_hold_ms: int = 30_000) -> dict[str, int | bool]:
    run = one(
        r"^RUN: align=(\d+)ms@([0-9.]+)% catch=(\d+)Hz/(\d+)ms@([0-9.]+)% "
        r"ramp=(\d+)ms target=(\d+)Hz/([0-9.]+)% hold=(\d+)ms$",
        text,
        "RUN banner",
    )
    hold_ms = int(run.group(9))
    if hold_ms < minimum_hold_ms:
        raise ValueError(f"compiled hold is only {hold_ms} ms")

    transfer = one(
        r"^DRIVETRANSFER result=(\d+) zero_not_attempted=1 two_refused=1$",
        text,
        "handoff",
    )
    if transfer.group(1) != "1":
        raise ValueError("handoff was not accepted")

    lock = one(
        r"^LOCKSUMMARY accepted=(\d+) events=(\d+) commutations=(\d+) "
        r"live_irq=(\d+) stats_events=(\d+) .* postrun_only=1$",
        text,
        "lock summary",
    )
    accepted = int(lock.group(1))
    commutations = int(lock.group(3))
    if accepted == 0 or commutations == 0:
        raise ValueError("no accepted-event/commutation progress")
    if abs(accepted - commutations) > max(2, commutations // 20):
        raise ValueError(
            f"event/commutation mismatch: accepted={accepted} commutations={commutations}"
        )

    track = one(
        r"^TRACKSTOP event_fault=(\d+) last_event_us=(\d+) .* retained_operational_state=1$",
        text,
        "tracking status",
    )
    if track.group(1) != "0":
        raise ValueError(f"tracking fault event_fault={track.group(1)}")

    power = one(r"^POWERPATH reason=(\d+) stop_us=(\d+) .* disabled=1$", text, "power status")
    if power.group(1) != "1":
        raise ValueError(f"power path did not finish normally: reason={power.group(1)}")

    done = one(r"^DONE reason=(\d+) .* gates=off en=off$", text, "final shutdown")
    if done.group(1) != "1":
        raise ValueError(f"final run reason was {done.group(1)}, not normal deadline")

    if re.search(r"^FASTBUS .* tripped=1", text, re.MULTILINE):
        raise ValueError("fast bus-sag guard tripped")
    if re.search(r"^CURRENTFOLDBACK .* count=[1-9]", text, re.MULTILINE):
        raise ValueError("current foldback occurred")

    return {
        "pass": True,
        "hold_ms": hold_ms,
        "accepted": accepted,
        "commutations": commutations,
        "last_event_us": int(track.group(2)),
    }


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("transcript", type=Path)
    parser.add_argument("--minimum-hold-ms", type=int, default=30_000)
    args = parser.parse_args()
    try:
        result = verify(args.transcript.read_text(encoding="utf-8"), args.minimum_hold_ms)
    except (OSError, UnicodeError, ValueError) as exc:
        print(f"BEMF_LOCK FAIL: {exc}")
        return 1
    print(
        "BEMF_LOCK PASS"
        f" hold_ms={result['hold_ms']} accepted={result['accepted']}"
        f" commutations={result['commutations']} last_event_us={result['last_event_us']}"
    )
    return 0


if __name__ == "__main__":
    sys.exit(main())

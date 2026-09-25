"""Refuse to analyse fields an image is known to have measured wrongly.

Why this exists (E295-E301). `480263F1` fed the raw-scan bus-depth observer
`base.bus_ref` -- the pre-run baseline, which does not droop -- while the guard
it was supposed to mirror judges against a ~207 ms EWMA, which does. The
observer's threshold therefore drifted shallower with load, by 1 ADC code at
rung 150 and 13.7 at rung 475, and every cross-rung comparison of `raw*_n` on
that image compared counts taken at different depths.

**The defect was known and disclosed before the analysis that depended on it.**
E296 states in as many words that the image carries none of the fixes and cannot
qualify, and then predeclares and fires a quantitative test on the defective
quantity anyway. Two independent reviews had to disassemble the ELF to establish
what one table lookup can now assert. The rule "diagnostic results never qualify
another image" was honoured for verdicts and violated for findings, and nothing
in the tooling could tell.

So: a table of images and the fields they cannot be read for. Keyed on the
`# elf_sha256` a capture already carries, so it needs no firmware change and
cannot be fooled by a filename.

This is deliberately a **denylist of measured defects**, not an allowlist of
approved images: an allowlist silently refuses every new build and would be
switched off within a day. The cost of a denylist is that a defect nobody has
found yet is not caught -- which is honest, because a tool cannot know what has
not been discovered.

Usage:
    import provenance
    provenance.check(capture_path, needs=("raw1_n",))   # raises on refusal
    provenance.defects_for(sha)                         # -> list[Defect]
"""
from __future__ import annotations

import dataclasses
import pathlib
import re


@dataclasses.dataclass(frozen=True)
class Defect:
    """One measured defect in one image, and the fields it invalidates."""

    sha256: str
    label: str
    fields: tuple[str, ...]
    why: str
    evidence: str


# Every entry must cite evidence that was *measured*, not argued.
DEFECTS: tuple[Defect, ...] = (
    Defect(
        sha256="366E781FDC345A1F9C8CAB61792FDA8CB64C8B88830EAA9DED2A39C5A329A544",
        label="480263F1.e286-rawdepth",
        fields=("raw0_n", "raw1_n", "raw2_n", "raw3_n",
                "raw0_run", "raw1_run", "raw2_run", "raw3_run"),
        why="raw_depth observed the raw scan against base.bus_ref (the pre-run "
            "baseline) instead of the guard's filtered EWMA, so its threshold "
            "drifted shallower with load: 1 ADC code at rung 150, 13.7 at 475. "
            "Cross-rung comparisons of these counts compare different depths.",
        evidence="objdump of BusDepth::observe @0x08000604 shows `movs r1,#125; "
                 "lsls r1,r1,#3` (SCALE=1000, pre-E291) and both observe call "
                 "sites in scan_pass load ref_bus/ref_vref from the same base. "
                 "Within rungs, corr(reference bias, raw1 rate) = +0.792 while "
                 "corr(hold_ma, raw1 rate) = -0.626 (E299).",
    ),
)

_BY_SHA = {d.sha256.upper(): d for d in DEFECTS}


class Refused(Exception):
    """Raised when a capture is asked for a field its image measured wrongly."""


def sha_of(capture: pathlib.Path) -> str | None:
    """The `# elf_sha256` header a capture records, or None if absent.

    A capture with no header is **not** treated as clean: the caller decides,
    because the pre-manifest fixture wrote no header at all and those captures
    are unattributable rather than trustworthy.
    """
    for line in capture.read_text(encoding="utf-8", errors="replace").splitlines()[:8]:
        m = re.match(r"#\s*elf_sha256\s+([0-9A-Fa-f]{64})", line.strip())
        if m:
            return m.group(1).upper()
    return None


def defects_for(sha: str | None) -> list[Defect]:
    return [] if sha is None else [d for d in (_BY_SHA.get(sha.upper()),) if d]


def check(capture: pathlib.Path, needs: tuple[str, ...], *, strict: bool = True) -> None:
    """Refuse if `capture`'s image is recorded as measuring any of `needs` wrongly.

    `strict` also refuses a capture with no recorded image identity, which is
    the right default for anything quantitative: an unattributable capture is
    not evidence about an image.
    """
    sha = sha_of(capture)
    if sha is None:
        if strict:
            raise Refused(
                f"{capture.name}: no `# elf_sha256` header, so its image is "
                f"unknown and {needs!r} cannot be attributed to any build. "
                f"Pass strict=False only to analyse pre-manifest captures, and "
                f"say so in the write-up."
            )
        return
    for d in defects_for(sha):
        bad = sorted(set(needs) & set(d.fields))
        if bad:
            raise Refused(
                f"{capture.name} was produced by {d.label} "
                f"(sha256 {sha[:16]}...), which is recorded as measuring "
                f"{', '.join(bad)} wrongly.\n"
                f"  why: {d.why}\n"
                f"  evidence: {d.evidence}\n"
                f"If the comparison is deliberately ABOUT the defect, read the "
                f"field directly rather than through this gate."
            )


def filter_usable(
    captures: list[pathlib.Path], needs: tuple[str, ...], *, strict: bool = True
) -> tuple[list[pathlib.Path], list[tuple[pathlib.Path, str]]]:
    """Split captures into usable and refused, so a caller can report both.

    Returning the refusals rather than dropping them is the point: a tool that
    silently narrows its own cohort is how the campaign lost track of which
    image produced which number in the first place.
    """
    ok, refused = [], []
    for c in captures:
        try:
            check(c, needs, strict=strict)
            ok.append(c)
        except Refused as e:
            refused.append((c, str(e).splitlines()[0]))
    return ok, refused


def main() -> int:
    import argparse
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    ap.add_argument("captures", nargs="*", default=[])
    ap.add_argument("--needs", default="raw1_n",
                    help="comma-separated fields to test for (default raw1_n)")
    ap.add_argument("--root", default="captures")
    ap.add_argument("--glob", default="**/*.txt")
    args = ap.parse_args()

    needs = tuple(f.strip() for f in args.needs.split(",") if f.strip())
    paths = ([pathlib.Path(c) for c in args.captures]
             or sorted(pathlib.Path(args.root).glob(args.glob)))
    ok, refused = filter_usable(paths, needs, strict=True)
    print(f"{len(DEFECTS)} recorded defective image(s); testing {needs} "
          f"over {len(paths)} capture(s)")
    print(f"  usable : {len(ok)}")
    print(f"  refused: {len(refused)}")
    by_reason: dict[str, int] = {}
    for _c, reason in refused:
        key = "no elf_sha256 header" if "no `# elf_sha256`" in reason else reason.split(" was produced by ")[-1]
        by_reason[key] = by_reason.get(key, 0) + 1
    for reason, n in sorted(by_reason.items(), key=lambda kv: -kv[1]):
        print(f"    {n:>5}  {reason}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

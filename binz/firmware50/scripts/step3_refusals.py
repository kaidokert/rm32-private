#!/usr/bin/env python3
"""ENV-34: inside step-3 sectors, what did the comparator read on each REFUSED edge?

Chain rows (image with the ENV-34 refusal log): kind 1 accepts (at_us = crossing us, step), kind 3
step-3 refusals (at_us = us since sector start, x_fine = live reads, oldest bit 0, count in bits
12..15; y_us = 1 too-early else filter; flag bit 6 = revisit-originated). Rows are in ISR order,
so refusals precede their sector's accept. Step 3 expects a FALLING comparator edge
(`COMP_POLARITY_INVERTED`), so a post-crossing read is bit = 0 -> printed 'P', pre -> 'p'.
Classifies each step-3 sector by its accepted interval (late > 1.25 x capture mean) and reports
whether any refused edge had its FIRST read post-crossing before 85 us (crossing present but
refused) vs all first reads pre (crossing not yet there). Refuses a capture with no kind-3 rows."""
import sys, statistics as st
for path in sys.argv[1:]:
    R = [list(map(int, l.split()[1:])) for l in open(path) if l.startswith("CHAIN ")]
    acc = [r for r in R if r[0] == 1]
    assert any(r[0] == 3 for r in R), f"REFUSED {path}: no refusal rows (image lacks the ENV-34 log)"
    m = st.mean(((q[1] - p[1]) & 0xFFFF) for p, q in zip(acc, acc[1:]))
    sectors, cur, prev_at = [], [], None
    for r in R:
        if r[0] == 3:
            cur.append(r)
        elif r[0] == 1:
            # Refusals are logged only while the step-3 crossing is sought, so the
            # group closes at the step-3 accept; every accept resets the group.
            if r[6] == 3 and prev_at is not None:
                sectors.append(((r[1] - prev_at) & 0xFFFF, cur, r[7] & 0x40))
            cur = []
            prev_at = r[1]
    def pat(x):
        n = (x >> 12) & 15
        return "".join("P" if ((x >> i) & 1) == 0 else "p" for i in range(min(n, 12)))
    late = [s for s in sectors if s[0] > 1.25 * m]; norm = [s for s in sectors if s[0] <= 1.1 * m]
    def early_post(s):
        return any(r[1] < 85 and ((r[3] >> 12) & 15) >= 1 and (r[3] & 1) == 0 and r[4] == 0 for r in s[1])
    print(f"{path.split('/')[-1]}: mean {m:.1f} us, step-3 sectors {len(sectors)}, late {len(late)}, normal {len(norm)}")
    print(f"  refusals/sector: late {st.mean(len(s[1]) for s in late) if late else 0:.2f}, normal {st.mean(len(s[1]) for s in norm) if norm else 0:.2f}")
    print(f"  sectors with a refused edge whose FIRST read was post-crossing before 85 us: late {sum(map(early_post, late))}/{len(late)}, normal {sum(map(early_post, norm))}/{len(norm)}")
    for iv, rs, rev in late[:6]:
        print(f"   LATE {iv} us{' (acc via revisit)' if rev else ''}: " + "  ".join(f"t{r[1]}{'E' if r[4] else 'F'}{'r' if r[7] & 0x40 else ''}:{pat(r[3])}" for r in rs))

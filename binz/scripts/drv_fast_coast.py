"""Verify opt-in coast cadence and actual timing; does not certify rotor lock."""
import re
from drv_capture import parse_dump


def verify(text, required=False):
    _, _, rows, hz, _, _ = parse_dump(text)
    if hz not in (2000, 8000) or (hz == 8000) != required:
        raise ValueError('coast cadence requires matching --fast-coast')
    if not required:
        return None
    if len(rows) != 500 or any('elapsed_us' not in r for r in rows):
        raise ValueError('fast coast needs all500 measured timestamps')
    gaps=[b['elapsed_us']-a['elapsed_us'] for a,b in zip(rows,rows[1:])]
    # Independent diagnostic admission: report actual scan spacing, reject
    # missing/overrun samples rather than label a requested8kHz as achieved.
    if not gaps or min(gaps)<125 or max(gaps)>150:
        raise ValueError('fast coast measured cadence outside125..150us')
    offsets=re.findall(r'^COASTCOMP row=(\d+) a_us=(\d+) b_us=(\d+) c_us=(\d+) from_scan_start=1$',text.replace('\r',''),re.M)
    if len(offsets)!=32 or [int(r[0]) for r in offsets]!=list(range(32)):
        raise ValueError('fast coast early comparator timing missing')
    for _,a,b,c in offsets:
        if not 0<int(a)<int(b)<int(c)<125:
            raise ValueError('fast coast early scan did not fit sample period')
    return dict(samples=len(rows),nominal_hz=hz,min_gap_us=min(gaps),max_gap_us=max(gaps),
                early_scan_max_us=max(int(r[3]) for r in offsets),lock_proven=False)

"""Optional dispatched-handler aggregates; not exclusive CPU costs or clean ZCs."""
import re

FIELDS = 'no_gate closed open_no_accept accepted stopped_or_unknown'.split()


def decode(text, *, required=False):
    lines = re.findall(r'^COMPPATH(?:\s[^\r\n]*)?$', text, re.M)
    if not lines and not required:
        return None
    pattern = (r'COMPPATH scope=dispatched first_actual_count=1 ' +
               ' '.join(name + r'=(\d+)' for name in FIELDS) + r' active=0')
    match = re.fullmatch(pattern, lines[0]) if len(lines) == 1 else None
    if not match:
        raise ValueError('invalid COMPPATH metadata/counts or unfinished handler')
    values = dict(zip(FIELDS, map(int, match.groups())))
    if any(value >= 0xffffffff for value in values.values()):
        raise ValueError('COMPPATH overflow or saturated counter')
    return dict(**values, dispatched=sum(values.values()))

"""Infer a logical-phase to physical-ADC permutation from saved pair probes."""
import itertools
import pathlib
import re
import sys


def infer(text):
    blocks = re.findall(r'PAIR id=(\d) source=(\d) sink=(\d) duty=60 n=(\d+) us=(\d+) reason=(\d+) bus0=(\d+) busmin=(\d+) en=0 moe=0\n(.*?)(?=PAIR id=|\Z)', text, re.S)
    measurements = []
    repeats = [0]*6
    for ident, source, sink, count, duration, reason, bus0, busmin, body in blocks:
        ident, source, sink, count, duration, reason, bus0, busmin = map(int, (ident,source,sink,count,duration,reason,bus0,busmin))
        if count != 16 or duration >= 2000 or reason or busmin*10 < bus0*7:
            raise ValueError('incomplete or guarded excitation')
        rows = re.findall(r'PAIR adc=(\d) zero=(\d+) delta_mean=(-?\d+) min=(-?\d+) max=(-?\d+)', body)
        if len(rows) != 3 or {int(r[0]) for r in rows} != {0,1,4}:
            raise ValueError('missing/duplicate ADC rows')
        measurements.append((source,sink,{int(r[0]): int(r[2]) for r in rows}))
        repeats[ident] += 1
    if min(repeats) < 2:
        raise ValueError('need two repeats of every directed pair')
    accepted = []
    for mapping in itertools.permutations([0,1,4]):
        good = True
        for source,sink,delta in measurements:
            a,b,c = delta[mapping[source]], delta[mapping[sink]], delta[mapping[3-source-sink]]
            # Large opposite-sign responses; inactive <10% of weaker active.
            good &= a > 100 and b < -100 and abs(c)*10 < min(abs(a),abs(b))
        if good:
            accepted.append(mapping)
    if len(accepted) != 1:
        raise ValueError(f'ambiguous/inconsistent mapping: {accepted}')
    return accepted[0]


if __name__ == '__main__':
    mapping = infer(pathlib.Path(sys.argv[1]).read_text())
    print('Unique phase A/B/C -> ADC mapping:', mapping)

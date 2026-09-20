"""Audit commanded source-sink PWM difference, NOT rotor angle or current.

Sine drives all three phases. Six-step drives a pair and floats the third;
equal pair voltage therefore does not imply equal torque or equal space vector.
Values exclude dead-time, switching drops, rotor BEMF and preload timing.
"""
import argparse
import json
import re
from pathlib import Path


def audit(source, sine_duty=65, observation_duty=80):
    match = re.search(r'const SINE_LUT: \[u8; 256\] = \[(.*?)\];', source, re.S)
    if not match:
        raise ValueError('SINE_LUT missing')
    lut = [int(v) for v in re.findall(r'\d+', match.group(1))]
    if len(lut) != 256:
        raise ValueError('expected 256 LUT entries')
    if not 1 <= sine_duty <= 100 or not 1 <= observation_duty <= 100:
        raise ValueError('duty outside existing 1..100 ceiling')
    spreads = []
    for theta in range(256):
        ccr = [6400*sine_duty*lut[(theta+85*i)&255]//255000 for i in range(3)]
        spreads.append(max(ccr)-min(ccr))
    applied = 6400*observation_duty//1000
    return dict(sine_duty_tenths=sine_duty, observation_duty_tenths=observation_duty,
                phase_bins=256, sine_pair_ccr_range=[min(spreads), max(spreads)],
                sixstep_pair_ccr=applied,
                pair_voltage_ratio_range=[applied/max(spreads), applied/min(spreads)],
                caveat='commanded source-sink difference only; not rotor alignment or torque')


if __name__ == '__main__':
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument('--sine-duty', type=int, default=65)
    p.add_argument('--observation-duty', type=int, default=80)
    args = p.parse_args()
    source = (Path(__file__).resolve().parents[1]/'examples/support/sine_table.rs').read_text()
    print(json.dumps(audit(source, args.sine_duty, args.observation_duty), indent=2))

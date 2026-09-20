"""Source-level floating-phase slope audit, not a measured rotor model."""
import json
import re
from pathlib import Path


def audit(source):
    values=re.search(r'const SINE_LUT: \[u8; 256\] = \[(.*?)\];',source,re.S)
    lut=list(map(int,re.findall(r'\d+',values.group(1))))
    pairs=[(0,1),(2,1),(2,0),(1,0),(1,2),(0,2)]
    rows=[]
    for step,(source_phase,sink) in enumerate(pairs,1):
        floating=3-source_phase-sink
        centers=[]
        for theta in range(256):
            v=[lut[(theta+85*p)&255] for p in range(3)]
            if v[source_phase]>v[floating]>v[sink] and abs(v[floating]-128)<=3:
                slope=lut[(theta+85*floating+1)&255]-lut[(theta+85*floating-1)&255]
                centers.append((theta,slope))
        if not centers or len({s>0 for _,s in centers})!=1:
            raise ValueError('ambiguous center slope')
        voltage_rises=centers[0][1]>0
        raw_post=not voltage_rises # neutral INP, phase INM, noninverted CSR
        rows.append(dict(step=step,source='ABC'[source_phase],sink='ABC'[sink],
            floating='ABC'[floating],center_bins=[t for t,_ in centers],
            voltage_rises=voltage_rises,raw_post_level=int(raw_post),
            reference_expected=int(step%2==1),matches=raw_post==(step%2==1)))
    return rows


if __name__=='__main__':
    source=(Path(__file__).resolve().parents[1]/'examples/support/sine_table.rs').read_text()
    print(json.dumps(audit(source),indent=2))

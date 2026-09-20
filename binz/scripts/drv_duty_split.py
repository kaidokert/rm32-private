"""Validate independent acquisition/segment duty without inferring waveform proof."""
import re


def verify(text, *, acquisition=None, bemf=None):
    rows=re.findall(r'^DUTYSPLIT .*$',text.replace('\r',''),re.M)
    if not rows and bemf is None:return None
    match=re.fullmatch(r'DUTYSPLIT acquisition=(\d+) bemf=(\d+) units=tenths_percent segment_fixed=1',rows[0]) if len(rows)==1 else None
    if not match:raise ValueError('missing/malformed duty split')
    a,b=map(int,match.groups())
    if not 40<=a<=62 or not 40<=b<=100:
        raise ValueError('duty split exceeds current firmware limits')
    if acquisition is not None and a!=acquisition or bemf is not None and b!=bemf:
        raise ValueError('duty split differs from request')
    return dict(acquisition=a,bemf=b)

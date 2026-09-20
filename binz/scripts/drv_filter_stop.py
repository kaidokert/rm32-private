"""Decode a retained pre-clear TIM2 source snapshot, not a lock verdict."""
import argparse
import json
import re
from pathlib import Path
from drv_capture import verify_off

FIELDS='sr dier ccer cr1 ccmr1 tisel cnt csr armed enabled nvic_enabled nvic_pending'.split()
def decode(text):
    text=text.replace('\r','')
    if text.count('FINALOFF\n')!=1:raise ValueError('finaloff boundary')
    body,final=text.split('FINALOFF\n');verify_off(final.encode())
    rows=re.findall(r'^FILTERSTOP .*$',body,re.M)
    if len(rows)!=1:raise ValueError('expected one source-stop snapshot')
    pattern='FILTERSTOP '+' '.join(f'{k}=(\\d+)' for k in FIELDS)+' before_source_stop=1'
    m=re.fullmatch(pattern,rows[0])
    if not m:raise ValueError('source-stop format')
    v=dict(zip(FIELDS,map(int,m.groups())))
    if any(n>0xffffffff for n in v.values()) or v['cnt']>65535 or any(v[k]>1 for k in FIELDS[8:]):
        raise ValueError('source-stop range')
    return dict(registers=v,capture_pending=bool(v['sr']&4),overcapture=bool(v['sr']&1024),
        peripheral_irq_enabled=bool(v['dier']&4),capture_enabled=bool(v['ccer']&16),
        rising=not bool(v['ccer']&32),counter_running=bool(v['cr1']&1),
        filter_code=v['ccmr1']>>12&15,ckd=v['cr1']>>8&3,ti2_source=v['tisel']>>8&15,
        raw_level=bool(v['csr']&(1<<30)),before_source_stop=True)

def main():
    ap=argparse.ArgumentParser(description=__doc__);ap.add_argument('capture',type=Path)
    args=ap.parse_args();print(json.dumps(decode(args.capture.read_text()),indent=2))
if __name__=='__main__':main()

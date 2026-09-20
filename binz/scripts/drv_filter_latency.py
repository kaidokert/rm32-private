"""First-raw-read mirror timing; latest capture, not original IRQ latency."""
import argparse
import json
import re
from pathlib import Path
from drv_capture import verify_off

def decode(text):
    text=text.replace('\r','')
    if text.count('FINALOFF\n')!=1:raise ValueError('finaloff boundary')
    body,final=text.split('FINALOFF\n');verify_off(final.encode())
    heads=re.findall(r'^FILTERLATENCY (.*)$',body,re.M)
    m=re.fullmatch(r'n=(\d+) latest_capture=1 authority=0 tick_half_us=1 prefix=1',heads[0]) if len(heads)==1 else None
    if not m or int(m[1])>24:raise ValueError('latency provenance/count')
    lines=re.findall(r'^FILTERLAT (.*)$',body,re.M)
    if len(lines)!=int(m[1]):raise ValueError('latency rows missing')
    fields='seq step epoch valid over capture before after raw pwm'.split()
    rows=[];last=0
    for line in lines:
        m=re.fullmatch(' '.join(k+r'=(\d+)' for k in fields),line)
        if not m:raise ValueError('latency format')
        v=dict(zip(fields,map(int,m.groups())))
        if v['seq']<=last or not 1<=v['step']<=6 or v['epoch']>0xffffffff or max(v[k] for k in ['valid','over','raw'])>1 or max(v[k] for k in ['capture','before','after'])>65535 or v['pwm']>2665:
            raise ValueError('latency range/order')
        last=v['seq'];bracket=(v['after']-v['before'])&65535
        if bracket>400:raise ValueError('raw read bracket exceeds200us')
        v['raw_bracket_us']=bracket/2
        v['latest_capture_age_us']=((v['before']-v['capture'])&65535)/2 if v['valid'] else None
        v['original_irq_latency_proven']=False
        rows.append(v)
    return rows

def main():
    ap=argparse.ArgumentParser(description=__doc__);ap.add_argument('capture',type=Path)
    args=ap.parse_args();print(json.dumps(decode(args.capture.read_text()),indent=2))
if __name__=='__main__':main()

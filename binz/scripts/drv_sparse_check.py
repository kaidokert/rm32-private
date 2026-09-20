"""Disabled sparse observer checks. No motor command; retain failed checks."""
import argparse
import re
from pathlib import Path
import serial
from drv_capture import send_line, read_available, verify_off
from drv_qualification_sparse import decode

LIMITS=(2,4,4,4,4,4,4,4)
PRELOADED=(0,0,0,0,0,1,16,32)

def verify(text):
    text=text.replace('\r','')
    if text.count('FINALOFF\n')!=1:raise ValueError('missing finaloff boundary')
    body,final=text.split('FINALOFF\n');verify_off(final.encode())
    baseline=re.findall(r'^SPARSEBASE max_us=(\d+) samples=16 subtracted=0$',body,re.M)
    if len(baseline)!=1:raise ValueError('missing timer baseline')
    lines=re.findall(r'^SPARSECHECK .*$',body,re.M)
    if len(lines)!=len(LIMITS)+1 or lines[-1]!='SPARSECHECK END disabled=1':
        raise ValueError('missing or unexpected sparse checks')
    maxima=[]
    for expected,line in enumerate(lines[:-1]):
        m=re.fullmatch(r'SPARSECHECK mode=(\d+) passed=16 total=16 max_us=(\d+) preloaded=(\d+) synthetic_duration=1 gate_authority=0',line)
        if not m or int(m[1])!=expected or int(m[3])!=PRELOADED[expected]:raise ValueError('sparse semantics failed')
        maxima.append(int(m[2]))
    if any(cost>limit for cost,limit in zip(maxima,LIMITS)):
        raise ValueError(f'sparse overhead refused: maxima={maxima}, limits={LIMITS}')
    wire=re.findall(r'^SPARSEWIRE expected_epoch=(\d+) synthetic=1 expected_n=16 expected_omitted=5$',body,re.M)
    if len(wire)!=1:raise ValueError('missing sparse wire check')
    decoded=decode(body,required=True,expected_epoch=int(wire[0]))
    if decoded['omitted_selected']!=5 or len(decoded['rows'])!=16:
        raise ValueError('sparse wire retention mismatch')
    for i,row in enumerate(decoded['rows'],5):
        expected=dict(accepted_before=i,entry_tick=i,duration_us=1 if i==20 else 41,
                      step=4 if i==20 else 3,accepted=i!=20,guard_overlap=i==19,stopped=i==20)
        if row!=expected:raise ValueError('sparse wire row mismatch')
    return dict(max_us=maxima,limits_us=LIMITS,outputs_off=True,synthetic_duration=True,
                timer_baseline_us=int(baseline[0]),baseline_subtracted=False,wire_roundtrip=True)

def main():
    ap=argparse.ArgumentParser(description=__doc__)
    ap.add_argument('--out',type=Path,required=True);args=ap.parse_args()
    with args.out.open('xb') as raw,serial.Serial('COM41',115200,timeout=.05) as port:
        data=bytearray()
        def command(c):
            send_line(port,c);b=read_available(port,.5);data.extend(b);raw.write(b);raw.flush()
        try:
            for c in ['off','p','i']:command(c)
            verify_off(data);command('sparsecheck')
        finally:
            raw.write(b'FINALOFF\n');data.extend(b'FINALOFF\n')
            for c in ['off','p','i']:command(c)
    print(verify(data.decode()))

if __name__=='__main__':main()

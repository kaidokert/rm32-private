"""Disabled direct-hook lifecycle, wire and fixed 2/4 us cost checks."""
import argparse
from pathlib import Path
import re
import serial
from drv_capture import send_line,read_available,verify_off
from drv_qualification_direct import decode


def cycle_report(text):
    """Supplementary raw bracket measurement, never subtraction or WCET proof."""
    lines=[s for s in text.replace('\r','').splitlines() if s.startswith('DIRECTCYCLES')]
    if not lines:return None
    if len(lines)!=15:raise ValueError('direct cycle rows missing/duplicate')
    maxima=[]
    for index,(preload,mode) in enumerate((p,m) for p in (0,16,32) for m in range(5)):
        m=re.fullmatch(rf'DIRECTCYCLES preload={preload} mode={mode} max_cycles=(\d+) valid=1 core_hz=64000000 reload=63999 subtraction=0 gate_authority=0',lines[index])
        if not m or not 0<int(m[1])<64000:raise ValueError('direct cycle clock/bounds invalid')
        maxima.append(int(m[1]))
    return dict(max_cycles=maxima,max_us=[c/64 for c in maxima],
                within_2_4_us=all(c<=(256 if i%5==2 else 128) for i,c in enumerate(maxima)),
                subtraction=False,powered_wcet_proven=False)


def verify(text,*,cycle_gate=False):
    text=text.replace('\r','')
    endings=list(re.finditer(r'^(?:> )?FINALOFF\n',text,re.M))
    if len(endings)!=1:raise ValueError('finaloff framing')
    body,final=text[:endings[0].start()],text[endings[0].end():];verify_off(final.encode())
    lines=re.findall(r'^DIRECTCHECK (.*)$',body,re.M)
    if len(lines)!=16 or lines[-1]!='END disabled=1':raise ValueError('direct check incomplete')
    cycles=cycle_report(body)
    maxima=[]
    for index,(preload,mode) in enumerate((p,m) for p in (0,16,32) for m in range(5)):
        m=re.fullmatch(rf'preload={preload} mode={mode} passed=16 total=16 max_us=(\d+) gate_authority=0',lines[index])
        if not m:raise ValueError('direct hook semantics')
        maxima.append(int(m[1]))
    wire=decode(body,True,expected_accepted=40)
    from drv_reject_time import decode as decode_late
    late=decode_late(body,campaign=False)
    if late is not None:
        if late['total']!=37 or [(r['accepted_before'],r['rejected_read_index']) for r in late['rows']]!=[(n,n%12) for n in range(32,40) if n%12]+[(40,11)]:
            raise ValueError('late rejection actual wire sequence mismatch')
    visits=2 if wire['wire_version']==2 else 1
    for row in wire['rows']:
        if (row['step']!=row['accepted_before']%6+1 or row['dispatched']!=visits or row['closed']!=0
                or row['open']!=visits or row['first_open']!=600 or row['last_open']!=600):
            raise ValueError('direct wire row mismatch')
        if wire['wire_version']==2 and row['rejected_read_indices']!=[row['accepted_before']%12]:
            raise ValueError('direct wire rejection position mismatch')
    partial=wire['partial']
    if (partial['dispatched'],partial['closed'],partial['open'],partial['first_open'],partial['last_open'])!=(1,0,1,777,777):
        raise ValueError('direct partial wire mismatch')
    if wire['wire_version']==2 and partial['rejected_read_indices']!=[11]:
        raise ValueError('direct partial rejection position mismatch')
    if cycle_gate:
        if cycles is None:raise ValueError('direct cycle gate requires raw cycle evidence')
        if not cycles['within_2_4_us']:raise ValueError('direct raw cycle cost exceeds 128/256 cycles')
    else:
        for index,cost in enumerate(maxima):
            limit=4 if index%5==2 else 2
            if cost>limit:raise ValueError(f'direct cost failed index={index}: {cost}>{limit} us; semantics/wire passed')
    return dict(maxima_us=maxima,wire_passed=True,outputs_off=True,powered_wcet_proven=False,cycles=cycles,
                cost_gate='raw_128_256_cycles' if cycle_gate else 'outer_2_4_us')


def main():
    ap=argparse.ArgumentParser(description=__doc__);ap.add_argument('--out',type=Path,required=True)
    ap.add_argument('--cycle-gate',action='store_true',help='use raw 128/256-cycle hook bracket; outer TIM17 includes extra observation work')
    args=ap.parse_args()
    with args.out.open('xb') as raw,serial.Serial('COM41',115200,timeout=.05) as port:
        data=bytearray()
        def command(c):
            send_line(port,c);b=read_available(port,1.0);data.extend(b);raw.write(b);raw.flush()
        try:
            for c in ['off','p','i']:command(c)
            verify_off(data);command('directcheck')
        finally:
            data.extend(b'FINALOFF\n');raw.write(b'FINALOFF\n')
            for c in ['off','p','i']:command(c)
    print(verify(data.decode(),cycle_gate=args.cycle_gate))


if __name__=='__main__':main()

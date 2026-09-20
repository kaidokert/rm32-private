"""Validate segment-local raw ADC sums. Does not estimate calibrated current."""
import argparse
import base64
import json
import re
import struct
import zlib
from pathlib import Path


def decode(text):
    streams=re.findall(r'^DMAFEEDBACK ([^\r\n]+)',text,re.M)
    period=101 # Legacy raw-sums format predates explicit alternate cadence.
    irq_guard=False
    queue_peak=0
    if streams:
        match=re.fullmatch(r'trigger_us=(101|201|209|226) acquisition_aged=1 experimental=1 irq_owned=1 max_us_u8=\d+ queue_peak=([0-8])(?: guard_irq=([01]))?',streams[0])
        if len(streams)!=1 or not match:raise ValueError('unsupported/duplicate ADC cadence')
        period=int(match[1])
        queue_peak=int(match[2]);irq_guard=match[3]=='1'
    starts=re.findall(r'^DMASTART ([^\r\n]+)',text,re.M)
    if starts and (not streams or len(starts)!=1 or starts[0] not in
                   [f'first_trigger_us={first} steady_trigger_us={period} preloaded_counter=1' for first in (1,101)]):
        raise ValueError('unsupported/duplicate ADC startup cadence')
    owners=re.findall(r'^DMAOWNER ([^\r\n]+)',text,re.M)
    if owners and owners!=['sequential_tim3=1 forced_release_required=1 refusal_code=11']:
        raise ValueError('unsupported/duplicate ADC ownership metadata')
    if owners and not streams:
        raise ValueError('sequential ADC ownership lacks stream metadata')
    if text.count('ADCSTATS uncalibrated=1 segment_only=1 current_center=2048') != 1:
        raise ValueError('missing/duplicate raw-stats header')
    rows={}
    for line in text.splitlines():
        if not line.startswith('S85 '):
            continue
        raw=base64.a85decode(line[4:])
        if len(raw)!=26 or zlib.crc32(raw[:22])!=int.from_bytes(raw[22:],'little'):
            raise ValueError('sum frame length/CRC')
        ch,nlo,nhi,flo,fhi,llo,lhi,total=struct.unpack('<7Hq',raw[:22])
        if ch not in range(5) or ch in rows:
            raise ValueError('invalid/duplicate sum channel')
        n=nlo|(nhi<<16); first=flo|(fhi<<16); last=llo|(lhi<<16)
        if n>6_000_000 or (n and ((last-first)&0xffffffff)!=period*(n-1)):
            raise ValueError('count/time inconsistency')
        lower,upper=(-2048*n,2047*n) if ch<3 else (0,4095*n)
        if not lower<=total<=upper or (not n and (first or last)):
            raise ValueError('sum outside ADC bounds')
        rows[ch]=(n,first,last,total)
    if set(rows)!=set(range(5)):
        raise ValueError('missing sum channels')
    if len({row[:3] for row in rows.values()})!=1:
        raise ValueError('mismatched channel windows')
    n,first,last,_=rows[0]
    deliveries=re.findall(r'^FEEDBACKFIRST ([^\r\n]+)',text,re.M)
    if deliveries:
        match=re.fullmatch(r'seen=([01]) initial_age_us=(\d+) decision_us=(\d+) previous_us=(\d+) acquired_us=(\d+) fault=(\d+)',deliveries[0])
        if len(deliveries)!=1 or not match:raise ValueError('invalid first feedback record')
        seen,initial,decision,previous,acquired,fault=map(int,match.groups())
        if initial>1000 or any(v>0xffffffff for v in (decision,previous,acquired)) or fault>12:
            raise ValueError('first feedback bounds')
        if not seen and any((decision,previous,acquired,fault)):
            raise ValueError('unseen first feedback has timing')
        if seen:
            if previous!=(-initial)&0xffffffff or (n and first!=acquired):
                raise ValueError('first feedback timestamp provenance')
            if fault==0 and (((decision-previous)&0xffffffff)>1000 or ((decision-acquired)&0xffffffff)>1000):
                raise ValueError('first feedback accepted stale data')
    feedback=re.search(r'^POWERFEEDBACK scans=(\d+) ',text,re.M)
    unaggregated=0
    lean_irq = text.count('LEANCORE r1 recorder=0 comp_max=0 com_max=0 control_progress=1 lean_irq=1') == 1
    if feedback:
        unaggregated=int(feedback[1])-n
        # IRQ mode can stop with FIFO frames plus one in-flight publication/
        # popped foreground frame. These are NOT represented by the sums.
        # The contiguous prefix is still CRC/time validated; no full-coverage claim.
        if lean_irq and int(feedback[1]) == 0:
            # Lean IRQ deliberately omits the legacy publication counter; the
            # S85 window remains independently count/time/CRC validated.
            unaggregated = 0
        elif not (0<=unaggregated<=queue_peak+1 if irq_guard else unaggregated==0):
            raise ValueError('sum/feedback count mismatch')
    elif irq_guard:
        raise ValueError('IRQ guard requires publication count')
    return dict(n=n,first_us=first,last_us=last,trigger_us=period,
                signed_raw_sums=[rows[i][3] for i in range(3)],
                bus_raw_sum=rows[3][3],vref_raw_sum=rows[4][3],
                calibrated_current=False,coverage_proven=False,segment_only=True,
                irq_guard=irq_guard,unaggregated_publications=unaggregated,
                sequential_tim3_declared=bool(owners))


def main():
    p=argparse.ArgumentParser(description=__doc__);p.add_argument('capture',type=Path)
    text=p.parse_args().capture.read_text()
    from drv_capture import parse_dump,verify_off
    parse_dump(text)
    end=text.rfind('COAST END')
    if end<0: raise ValueError('missing final capture')
    verify_off(text[end+len('COAST END'):].encode())
    print(json.dumps(decode(text),indent=2))


if __name__=='__main__': main()

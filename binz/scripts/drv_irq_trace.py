"""Decode CRC-protected reference-handler read traces (not a lock classifier)."""
import argparse
import base64
import struct
import zlib
import re
from pathlib import Path

FIELDS='seq us step pwm_cnt gate_count avg reads first last pending masked accepts cost_us rising'.split()


def decode(text):
    reads=re.findall(r'^COMPREAD ([^\r\n]+)',text,re.M)
    if reads and reads!=['inline_adapter=1 sample_count_unchanged=1 signal_cached=0']:
        raise ValueError('invalid comparator read implementation provenance')
    modes=re.findall(r'^COMPMODE ([^\r\n]+)',text,re.M)
    if modes and modes!=['cached_per_call=1 signal_cached=0 safety_cached=0']:
        raise ValueError('invalid cached comparator mode provenance')
    windows=re.findall(r'^IRQWINDOW ([^\r\n]+)',text,re.M)
    tail=bool(windows)
    capacity=None
    if tail:
        match=re.fullmatch(r'mode=tail time_bits=32 capacity=(24|32) appended=us_hi',windows[0]) if len(windows)==1 else None
        if not match:raise ValueError('invalid IRQ tail window metadata')
        capacity=int(match[1])
    result=[]
    for line in text.splitlines():
        if not line.startswith('I85 '):
            continue
        raw=base64.a85decode(line[4:])
        if len(raw)!=(34 if tail else 32) or zlib.crc32(raw[:-4])!=int.from_bytes(raw[-4:],'little'):
            raise ValueError('IRQ trace length/CRC')
        words=struct.unpack('<15H' if tail else '<14H',raw[:-4])
        r=dict(zip(FIELDS,words[:14]))
        if tail:r['us']+=words[14]<<16
        if r['accepts']:
            r['outcome']='accepted'
        elif r['gate_count']==65535:
            r['outcome']='no_gate_read'
        elif r['gate_count'] <= r['avg']//2:
            r['outcome']='blank_gate'
        elif r['reads'] and r['last']!=r['rising']:
            r['outcome']='persistence_reject'
        else:
            r['outcome']='unresolved'
        result.append(r)
    header=re.search(r'^IRQTRACE n=(\d+) drop=(\d+) ',text,re.M)
    if header and len(result)!=int(header[1]):
        raise ValueError('IRQ trace count mismatch')
    if tail:
        if not header or len(re.findall(r'^IRQTRACE n=',text,re.M))!=1:
            raise ValueError('IRQ tail needs singleton count header')
        if len(result)>capacity or (int(header[2]) and len(result)!=capacity):
            raise ValueError('IRQ tail capacity/accounting')
        if any(b['us']<a['us'] or (b['seq']-a['seq'])%65536==0 for a,b in zip(result,result[1:])):
            raise ValueError('IRQ tail chronology/sequence')
    return result


def gate_accept_timing(text):
    """Join same-observation-clock tail records, not guard-clock timestamps.

    Gate count precedes filtering; this_zc follows it. Their difference includes
    bookkeeping and preemption, not just the first-to-last comparator aperture.
    A refused event has no accepted row and is deliberately not joined here.
    """
    from drv_accepted_events import decode_windows
    from drv_sustained_report import summarize
    summarize(text)
    if not re.search(r'^IRQWINDOW mode=tail time_bits=32 ',text,re.M):
        raise ValueError('full-width IRQ tail required for timestamp join')
    accepted=decode_windows(text)['tail']
    result=[]
    for r in decode(text):
        if not r['accepts']:continue
        matches=[e for e in accepted if e['step']==r['step']
                 and r['us']<=e['us']<=r['us']+r['cost_us']]
        if len(matches)!=1 or r['accepts']!=1 or r['gate_count']==65535:
            raise ValueError('ambiguous IRQ/accepted-tail join')
        e=matches[0]
        ticks=(e['reference_interval_ticks']-r['gate_count'])%65536
        if ticks>2*r['cost_us']+2:
            raise ValueError('reference counter bracket exceeds handler wall bracket')
        result.append(dict(seq=r['seq'],step=r['step'],reads=r['reads'],
                           gate_to_accept_us=ticks/2,handler_cost_us=r['cost_us']))
    return dict(rows=result,pure_filter_duration=False,preemption_included=True,
                refused_event_included=False,independent_crossing_time=False)


def rejection_cadence(text):
    """Adjacent dispatched rejection visits, never inferred physical edges.

    Require full-width timestamps and consecutive sequence numbers. Do not
    bridge sector changes, accepted visits, or missing handlers. A carrier-like
    cadence can be ordinary PWM triggering; it does not identify preemption.
    """
    if not re.search(r'^IRQWINDOW mode=tail time_bits=32 ',text,re.M):
        raise ValueError('full-width IRQ tail required for rejection cadence')
    rows=decode(text)
    pairs=[]
    for a,b in zip(rows,rows[1:]):
        if ((b['seq']-a['seq'])%65536!=1 or a['step']!=b['step']
                or a['outcome']!='persistence_reject'
                or b['outcome']!='persistence_reject'):
            continue
        pairs.append(dict(step=a['step'],first_seq=a['seq'],last_seq=b['seq'],
                          entry_gap_us=b['us']-a['us'],
                          previous_handler_us=a['cost_us']))
    return dict(pairs=pairs,window='IRQ-conditioned tail',
                physical_edge_intervals=False,preemption_proven=False,
                uncensored_cycle_distribution=False)


def evidence_summary(text):
    """Describe observable transitions without inventing pre-IRQ history.

    An equal first/last pair does not exclude an intervening excursion. Even
    unequal endpoints do not identify a rotor crossing rather than switching.
    """
    heads=re.findall(r'^IRQTRACE n=(\d+) drop=(\d+) ',text,re.M)
    if len(heads)!=1: raise ValueError('missing/duplicate IRQ trace header')
    rows=decode(text);count,dropped=map(int,heads[0])
    changed=accepted_changed=accepted_unresolved=unread=0
    for r in rows:
        if r['reads']==0:
            if (r['first'],r['last'])!=(2,2): raise ValueError('invalid unread sentinels')
            unread+=1
        else:
            if r['first'] not in (0,1) or r['last'] not in (0,1):
                raise ValueError('invalid comparator levels')
            if r['reads']==1 and r['first']!=r['last']:
                raise ValueError('one read cannot change level')
        different=r['reads']>=2 and r['first']!=r['last']
        changed+=int(different)
        accepted_changed+=int(bool(r['accepts']) and different)
        accepted_unresolved+=int(bool(r['accepts']) and not different)
    return dict(retained_handlers=count,omitted_handlers=dropped,
                endpoint_change_observed=changed,handlers_without_level_read=unread,
                accepted_with_endpoint_change=accepted_changed,
                accepted_without_observed_endpoint_change=accepted_unresolved,
                independent_window_denominator_available=False,qzc_measured=False,
                window='tail' if re.search(r'^IRQWINDOW ',text,re.M) else 'prefix',
                limitation='IRQ-conditioned retained window; no pre-trigger level history, intermediate read sequence, or independent sensing-window count. Equal endpoints are inconclusive.')


if __name__=='__main__':
    p=argparse.ArgumentParser(description=__doc__)
    p.add_argument('capture',type=Path)
    p.add_argument('--evidence-summary',action='store_true')
    p.add_argument('--rejection-cadence',action='store_true')
    args=p.parse_args()
    text=args.capture.read_text()
    if args.rejection_cadence:
        import json
        print(json.dumps(rejection_cadence(text),indent=2))
    elif args.evidence_summary:
        import json
        print(json.dumps(evidence_summary(text),indent=2))
    else:
        for row in decode(text): print(row)

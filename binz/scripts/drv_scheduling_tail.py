"""Decode bounded software IRQ chronology; never infer hardware edge latency."""
import re
from drv_driven_run import records


def stack_ids(value):
    ids=[]
    while value:
        ids.append(value & 7)
        value >>= 3
    if len(ids)>6 or any(i not in range(1,7) for i in ids) or len(set(ids))!=len(ids):
        raise ValueError('invalid scheduling nesting stack')
    return list(reversed(ids))


def decode(text,required=False):
    headers=re.findall(r'^SCHEDTAIL ([^\r\n]+)',text,re.M)
    rows=records(text,'ST85',4)
    if not headers and not rows and not required:return None
    pattern=(r'count=(\d+) retained=(\d+) omitted=(\d+) elapsed_us=(\d+) '
             r'max_gap_us=(\d+) started=([01]) active=0 fault=([0-4]) '
             r'capacity=64 row_words=4 first_guard_origin=1 shared_cpu_clock=1 '
             r'physical_edges=0 blackout_watchdog=0')
    m=re.fullmatch(pattern,headers[0]) if len(headers)==1 else None
    if not m:raise ValueError('missing/unknown scheduling header')
    count,n,omitted,elapsed,gap,started,fault=map(int,m.groups())
    if (count>0xffffffff or n!=min(count,64) or omitted!=max(0,count-64)
            or len(rows)!=n or elapsed>600_000_000 or gap>65535
            or (not started and (count or elapsed or gap or fault))
            or (not fault and gap>1000)):
        raise ValueError('inconsistent scheduling metadata')
    decoded=[];previous=None;last=0;stopped=False
    for lo,hi,pl,ph in rows:
        at=lo | hi<<16;packed=pl | ph<<16
        stack=packed & ((1<<18)-1);kind=(packed>>18)&3;ident=(packed>>20)&7
        if packed>>23 or kind>2 or at>elapsed or stopped:
            raise ValueError('invalid scheduling row')
        ids=stack_ids(stack)
        if kind==0:
            if not ids or ids[-1]!=ident:raise ValueError('invalid scheduling entry')
            before=stack>>3
        elif kind==1:
            if ident not in range(1,7):raise ValueError('invalid scheduling exit')
            before=(stack<<3)|ident;stack_ids(before)
        else:
            if ident:raise ValueError('invalid scheduling stop')
            before=stack;stopped=True
        if previous is not None and (before!=previous or not last<=at<=last+1000):
            raise ValueError('discontinuous scheduling history')
        if previous is None and not omitted and (before or at>1000):
            raise ValueError('invalid scheduling origin')
        previous=stack;last=at
        decoded.append(dict(us=at,kind=kind,vector=ident,active_stack=ids))
    if not fault and started and (not stopped or last!=elapsed):
        raise ValueError('scheduling history not frozen at stop')
    if fault and stopped:raise ValueError('fault after frozen scheduling stop')
    if required and (not started or fault):raise ValueError('scheduling observer not valid')
    return dict(rows=decoded,count=count,omitted=omitted,elapsed_us=elapsed,fault=fault,
                valid=bool(started and not fault),physical_edge_latency=False,
                foreground_masks_measured=False,complete_history=not omitted)


def verify_cpu(text,required=False):
    result=decode(text,required=required)
    if result is None:return None
    from drv_cpu_meter import decode as cpu_decode
    cpu=cpu_decode(text)
    stopped=re.findall(r'^CPUUNION .* stopped_depth=([0-6]) ',text,re.M)
    if (not result['valid'] or not cpu['valid'] or cpu['kind']!='irq_union'
            or len(stopped)!=1 or result['elapsed_us']!=cpu['elapsed_us']
            or result['count']!=2*cpu['calls'][1]-int(stopped[0])+1
            or len(result['rows'][-1]['active_stack'])!=int(stopped[0])):
        raise ValueError('scheduling/CPU epoch or event count mismatch')
    return result

"""Strict diagnostic-only qualification-window decoding, not motor qualification."""
import base64
import re
import struct
import zlib


def words(encoded,width):
    raw=base64.a85decode(encoded)
    size=width*2
    if len(raw)!=size+4 or zlib.crc32(raw[:size])!=int.from_bytes(raw[size:],'little'):
        raise ValueError('qualification CRC/length')
    return struct.unpack('<'+'H'*width,raw[:size])


def window(values):
    calls,overlap,maximum,saturated=values
    if overlap>calls or saturated>1 or (not calls and (overlap or maximum or saturated)):
        raise ValueError('qualification window counters')
    return dict(calls=calls,guard_overlap_calls=overlap,max_call_us=maximum,saturated=bool(saturated))


def decode(text,required=False):
    heads=re.findall(r'^QUALWINDOW ([^\r\n]+)',text,re.M)
    rows=re.findall(r'^QW85 ([^\r\n]+)',text,re.M)
    pending=re.findall(r'^QP85 ([^\r\n]+)',text,re.M)
    if not heads and not rows and not pending and not required:return None
    m=re.fullmatch(r'n=(\d+) omitted=(\d+) invalid=0 frozen=1 capture=([01]) dispatch_body=1 edge_time=0',
                   heads[0]) if len(heads)==1 else None
    if not m:raise ValueError('qualification header/invalid buffer')
    n,omitted,capture=map(int,m.groups())
    if not 0<=n<=16 or omitted+n>0xffffffff or (omitted and n!=16):
        raise ValueError('qualification count bounds')
    if not capture:
        if required or rows or pending:raise ValueError('qualification capture absent')
        return dict(captured=False,rows=None)
    if len(rows)!=n or len(pending)!=1:raise ValueError('qualification record counts')
    if required and not n:raise ValueError('qualification path unused')
    decoded=[]
    for i,line in enumerate(rows):
        lo,hi,step,accepted,stopped,*counts=words(line,9)
        ordinal=lo|(hi<<16)
        if not 1<=step<=6 or accepted>1 or stopped>1 or not (accepted or stopped):
            raise ValueError('qualification row flags')
        if stopped and i!=n-1:raise ValueError('qualification rows after stop')
        if ordinal!=omitted+i+accepted:raise ValueError('qualification ordinal')
        w=window(counts)
        if not w['calls']:raise ValueError('qualification empty completed row')
        decoded.append(dict(ordinal=ordinal,step=step,accepted=bool(accepted),stopped=bool(stopped),**w))
    p=window(words(pending[0],4))
    if decoded and decoded[-1]['stopped'] and p['calls']:
        raise ValueError('qualification pending after final call')
    return dict(captured=True,rows=decoded,pending=p,omitted=omitted,
                dispatch_body=True,physical_edge_time=False,
                preentry_latency_measured=False,overlap_is_tick_count=False)

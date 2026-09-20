"""Strict optional completed-call tail; never physical edge arrival times."""
import base64
import re
import struct
import zlib


def decode(text,required=False):
    heads=re.findall(r'^COMPDECISION ([^\r\n]+)',text,re.M)
    records=re.findall(r'^CD85 ([^\r\n]+)',text,re.M)
    if not heads and not required and not records:return None
    pattern=r'n=(\d+) total=(\d+) omitted=(\d+) invalid=0 frozen=1 active=0 capture=([01]) entry_not_edge=1 wire=d85-v1'
    m=re.fullmatch(pattern,heads[0]) if len(heads)==1 else None
    if not m:raise ValueError('decision metadata or invalid/live buffer')
    n,total,omitted,capture=map(int,m.groups())
    if not 0<=n<=32 or not n<=total<=0xffffffff or omitted!=total-n or n!=min(32,total):
        raise ValueError('decision count/omission accounting')
    from drv_comp_paths import decode as paths_decode
    paths=paths_decode(text)
    if paths is not None and paths['dispatched']!=total:
        raise ValueError('decision/aggregate epoch accounting')
    if not capture:
        if records or required:raise ValueError('decision capture not requested')
        return dict(rows=None,omitted=omitted,captured=False,physical_edge_timestamps=False)
    if required and not n:raise ValueError('decision path unused')
    rows=[]
    for encoded in records:
        raw=base64.a85decode(encoded)
        if len(raw)!=20 or zlib.crc32(raw[:16])!=int.from_bytes(raw[16:],'little'):
            raise ValueError('decision CRC/length')
        us,seq,packed,meta=struct.unpack('<4I',raw[:16])
        step,path,present=meta&255,(meta>>8)&255,meta>>16
        count,avg=packed&65535,packed>>16
        if not 1<=step<=6 or path>4 or present>1 or (not present and count):
            raise ValueError('decision row fields')
        if seq!=omitted+len(rows)+1 or (rows and us<rows[-1]['us']):
            raise ValueError('decision chronology')
        if path==0 and present or path in (1,2,3) and not present:
            raise ValueError('decision count presence')
        if path==1 and count>avg//2 or path in (2,3) and count<=avg//2:
            raise ValueError('decision gate classification')
        rows.append(dict(us=us,seq=seq,step=step,path=path,count=count if present else None,average=avg))
    if len(rows)!=n:raise ValueError('decision row count')
    return dict(rows=rows,omitted=omitted,physical_edge_timestamps=False)

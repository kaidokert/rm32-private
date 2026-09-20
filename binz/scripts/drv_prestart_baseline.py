"""Raw prestart baseline provenance, not zero-current calibration."""
import base64
import re
import struct
import zlib


def decode(text):
    heads=re.findall(r'^PREBASE (.*)$',text,re.M)
    records=re.findall(r'^BZ85 (.*)$',text,re.M)
    epochs=re.findall(r'^BASEEPOCH (.*)$',text,re.M)
    acquisition=re.findall(r'^BASEACQ (.*)$',text,re.M)
    if not heads and not records and not epochs and not acquisition: return None
    mode=dict(dma=False,trigger_us=None,first_us=None,irq_consumed=False)
    if acquisition:
        match=re.fullmatch(r'dma=1 trigger_us=(101|201|209|226) first_us=(1|101) channels=0,1,4,6,13 irq_consumed=0 offsets_applied=0',acquisition[0])
        if len(acquisition)!=1 or not match:raise ValueError('unsupported/duplicate baseline acquisition')
        period,first=map(int,match.groups())
        mode=dict(dma=True,trigger_us=period,first_us=first,irq_consumed=False)
    if len(heads)!=1 or len(records)!=5: raise ValueError('baseline header/record count')
    h={k:int(v) for k,v in re.findall(r'(\w+)=(\d+)',heads[0])}
    if (set(h)!={'status','n_target','elapsed_us','entry_same_epoch','stationary_verified','offsets_applied','guard_settings_unchanged'}
            or h['status'] not in (1,2) or h['n_target']!=128 or h['entry_same_epoch'] not in (0,1,2)
            or h['stationary_verified']!=0 or h['offsets_applied']!=0 or h['guard_settings_unchanged']!=1):
        raise ValueError('unsupported baseline metadata')
    rows=[]
    for expected,payload in zip([4,1,0,6,13],records):
        raw=base64.a85decode(payload.strip())
        if len(raw)!=24 or zlib.crc32(raw[:20])!=int.from_bytes(raw[20:],'little'):
            raise ValueError('baseline CRC/length')
        ch,n,lo,hi,s0,s1,s2,s3,minimum,maximum=struct.unpack('<10H',raw[:20])
        total=lo+(hi<<16);squares=s0+(s1<<16)+(s2<<32)+(s3<<48)
        if ch!=expected or n>128: raise ValueError('baseline channel/count')
        if n:
            if not 0<=minimum<=maximum<=4095 or not n*minimum<=total<=n*maximum or not total*total<=n*squares or not n*minimum**2<=squares<=n*maximum**2:
                raise ValueError('baseline impossible moments')
        elif (total,squares,minimum,maximum)!=(0,0,4095,0): raise ValueError('invalid empty baseline')
        rows.append(dict(channel=ch,n=n,sum=total,squares=squares,minimum=minimum,maximum=maximum))
    if h['status']==2 and (h['elapsed_us']>=50000 or any(r['n']!=128 for r in rows)):
        raise ValueError('incomplete baseline marked complete')
    epoch=None
    if epochs:
        if len(epochs)!=1:raise ValueError('duplicate baseline epoch')
        epoch={k:int(v) for k,v in re.findall(r'(\w+)=(\d+)',epochs[0])}
        if (set(epoch)!={'recovery_checked','recovery_matches','initial_records_only'}
                or epoch['initial_records_only']!=1 or epoch['recovery_checked'] not in (0,1)
                or epoch['recovery_matches'] not in (0,1)
                or epoch['recovery_matches']>epoch['recovery_checked']):
            raise ValueError('invalid baseline epoch metadata')
        if re.search(r'^REENTRY result=7 ',text,re.M) and epoch!={'recovery_checked':1,'recovery_matches':0,'initial_records_only':1}:
            raise ValueError('initial baseline not revoked for recovery')
    return dict(header=h,channels=rows,epoch=epoch,acquisition=mode,calibrated_current=False)

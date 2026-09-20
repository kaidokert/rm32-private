"""Decode early PWM launch coverage; never certify aperture/current accuracy."""
import base64
import re
import struct
import zlib


def decode(text):
    heads=re.findall(r'^ADCLAUNCH (.*)$',text,re.M)
    if len(heads)!=1: raise ValueError('missing/duplicate ADC launch header')
    expected=dict(bins=8,period_ticks=6400,limit=192,early_window=1,
                  aperture_known=0,joint_sector_coverage=0)
    if {k:int(v) for k,v in re.findall(r'(\w+)=(\d+)',heads[0])}!=expected:
        raise ValueError('unsupported ADC launch metadata')
    rows=[]
    for payload in re.findall(r'^AL85 (.*)$',text,re.M):
        raw=base64.a85decode(payload.strip())
        if len(raw)!=26 or zlib.crc32(raw[:22])!=int.from_bytes(raw[22:],'little'):
            raise ValueError('ADC launch length/CRC')
        phase,attempted,rejected,*bins=struct.unpack('<11H',raw[:22])
        if phase!=len(rows) or attempted>192 or rejected>attempted or sum(bins)+rejected!=attempted:
            raise ValueError('ADC launch order/count conservation')
        rows.append(dict(phase=phase,attempted=attempted,rejected=rejected,bins=bins))
    if len(rows)!=3: raise ValueError('ADC launch channel count')
    return dict(channels=rows,current_accuracy_qualified=False,
                scope='primary segment first 192 launch attempts per channel; not aperture or joint sector coverage')

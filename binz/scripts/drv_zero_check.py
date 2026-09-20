"""Capture/decode disabled, same-reader zero statistics. Never applies offsets."""
import argparse
import base64
import json
import math
from pathlib import Path
import re
import struct
import zlib
from drv_capture import send_line,read_available,verify_off


def decode(text):
    heads=re.findall(r'^ZEROCHK ([^\r\n]+)',text,re.M)
    if len(heads)!=1: raise ValueError('missing/duplicate zero header')
    h={k:int(v) for k,v in re.findall(r'(\w+)=(\d+)',heads[0])}
    if (h.get('result')!=0 or h.get('disabled')!=1 or h.get('same_powered_reader')!=1
        or h.get('applied_calibration')!=0 or not 0<h.get('elapsed_us',0)<200300):
        raise ValueError('incomplete/unsafe zero acquisition')
    windows=h.get('windows',1)
    if windows not in (1,2) or (windows==2 and h.get('same_wake')!=1): raise ValueError('zero window metadata')
    rows=[];second=[]
    for line in text.splitlines():
        if not line.startswith(('Z85 ','ZB85 ')): continue
        is_second=line.startswith('ZB85 ')
        raw=base64.a85decode(line.split(' ',1)[1])
        if len(raw)!=24 or zlib.crc32(raw[:20])!=int.from_bytes(raw[20:],'little'):
            raise ValueError('zero length/CRC')
        w=struct.unpack('<10H',raw[:20]);ch,n=w[:2]
        total=w[2]|w[3]<<16
        squares=sum(w[4+i]<<(16*i) for i in range(4));low,high=w[8:]
        if (n!=512 or not 0<=low<=high<=4095 or not n*low<=total<=n*high
            or not n*low*low<=squares<=n*high*high or squares*n<total*total):
            raise ValueError('invalid zero moments')
        mean=total/n
        (second if is_second else rows).append(dict(channel=ch,n=n,sum=total,squares=squares,min=low,max=high,
                         mean_raw=mean,sd_raw=math.sqrt((squares-total*total/n)/(n-1))))
    if [r['channel'] for r in rows]!=[4,1,0,6,13]: raise ValueError('zero channel order/count')
    if windows==2 and [r['channel'] for r in second]!=[4,1,0,6,13]: raise ValueError('second zero channel order/count')
    if windows==1 and second: raise ValueError('unannounced second window')
    verify_off(text[max(text.rfind('\nZ85 '),text.rfind('\nZB85 ')):].encode())
    return dict(header=h,channels=rows,second_channels=second,
                same_wake_delta_raw=[b['mean_raw']-a['mean_raw'] for a,b in zip(rows,second)],calibration_applied=False)


def normalized_drift(data, vcal):
    """Voltage-domain offset drift, NOT a calibrated powered-current reading.

    vcal must come from this MCU's factory VREFINT calibration at 3000 mV.
    Ratios of window means do not capture within-window voltage covariance.
    """
    if not isinstance(vcal,int) or isinstance(vcal,bool) or not 1<=vcal<=4095:
        raise ValueError('invalid factory VREFINT calibration')
    windows=[data['channels']]
    if data['second_channels']: windows.append(data['second_channels'])
    result=[]
    for rows in windows:
        reference=rows[4]['mean_raw']
        if reference<=0: raise ValueError('zero VREFINT cannot normalize voltage')
        vdda=3000*vcal/reference
        result.append(dict(vdda_mv=vdda,
                           csa_mean_mv=[r['mean_raw']*vdda/4096 for r in rows[:3]]))
    delta=([b-a for a,b in zip(result[0]['csa_mean_mv'],result[1]['csa_mean_mv'])]
           if len(result)==2 else [])
    return dict(vcal=vcal,windows=result,csa_delta_mv=delta,
                summed_output_drift_equivalent_ma_at_70mv_per_a=(sum(delta)*1000/70 if delta else None),
                powered_current_qualified=False,
                caveat='Ratio of window means; includes CSA bias drift; not a confidence bound or PSU current.')


def main():
    ap=argparse.ArgumentParser(description=__doc__)
    ap.add_argument('--out',type=Path)
    ap.add_argument('--input',type=Path,help='decode an existing raw capture without opening UART')
    ap.add_argument('--port',default='COM41')
    ap.add_argument('--vcal',type=int,help='this MCU factory VREFINT count at 3.0 V; enables voltage-domain report')
    args=ap.parse_args()
    if args.input:
        data=decode(args.input.read_text())
        if args.vcal is not None: data['voltage_normalization']=normalized_drift(data,args.vcal)
        print(json.dumps(data,indent=2));return
    if args.out is None: ap.error('--out is required for a live capture')
    if args.out.exists(): ap.error('refusing to overwrite raw capture')
    import serial
    raw=b''
    with serial.Serial(args.port,115200,timeout=.05) as port:
        raw+=read_available(port,.3)
        try:
            for command in ['off','p','i']:
                send_line(port,command);raw+=read_available(port,.25)
            verify_off(raw)
            send_line(port,'cap1');ack=read_available(port,.25);raw+=ack
            if b'CAPTURE armed' not in ack: raise ValueError('missing capture acknowledgement')
            send_line(port,'zerocheck');raw+=read_available(port,1.0)
        finally:
            for command in ['off','cap0','stack','p','i']:
                send_line(port,command);raw+=read_available(port,.25)
            args.out.write_bytes(raw)
    data=decode(raw.decode(errors='replace'))
    if args.vcal is not None: data['voltage_normalization']=normalized_drift(data,args.vcal)
    print(json.dumps(data,indent=2))


if __name__=='__main__': main()

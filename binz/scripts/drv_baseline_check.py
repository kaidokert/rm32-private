"""Idle prestart-baseline refusal/completion check; no motor command."""
import argparse
import re
from pathlib import Path
import serial
from drv_capture import send_line,read_available,verify_off
from drv_prestart_baseline import decode

def verify_cases(text,tim15=False,startup=False):
    verify_off(text[text.rfind('FINALOFF\n'):].encode())
    cases=re.findall(r'^BASECHECK case=(\d+) passed=1 disabled=1 gates_commanded=0\n(.*?)(?=^BASECHECK|^FINALOFF|\Z)',text,re.M|re.S)
    if [int(c) for c,_ in cases]!=list(range(4)):raise ValueError('baseline cases failed/missing')
    results=[]
    for case,body in cases:
        marker=('ADCTRIGGER timer=15 extsel=4 continuous_startup=1 backend_only=0 startup_vsenc_valid=0 startup_neutral_valid=0'
                if startup else 'ADCTRIGGER timer=15 extsel=4 continuous_startup=0 backend_only=1')
        if body.count(marker)!=(1 if tim15 else 0):raise ValueError('ADC trigger backend mismatch')
        result=decode(body);h=result['header']
        if h['status']!=(2 if case=='3' else 1):raise ValueError('wrong baseline outcome')
        if case=='3' and h['entry_same_epoch']!=2:raise ValueError('rewake retained baseline')
        results.append(dict(case=int(case),elapsed_us=h['elapsed_us'],counts=[r['n'] for r in result['channels']]))
    return results

def main():
    p=argparse.ArgumentParser(description=__doc__);p.add_argument('--out',type=Path,required=True)
    p.add_argument('--tim15',action='store_true',help='require exact TIM15 backend metadata in each case')
    p.add_argument('--startup',action='store_true',help='require the timed-startup build metadata (not proof of startup drive)')
    args=p.parse_args()
    with args.out.open('xb') as raw:
        data=bytearray()
        with serial.Serial('COM41',115200,timeout=.05) as port:
            def command(line,wait=.4):
                send_line(port,line);chunk=read_available(port,wait)
                data.extend(chunk);raw.write(chunk);raw.flush()
            try:
                for line in ['off','p','i']:command(line)
                verify_off(bytes(data));command('basecheck',1.5)
            finally:
                data.extend(b'FINALOFF\n');raw.write(b'FINALOFF\n')
                for line in ['off','p','i']:command(line)
        for result in verify_cases(data.decode().replace('\r',''),args.tim15 or args.startup,args.startup):print(result)


if __name__=='__main__':main()

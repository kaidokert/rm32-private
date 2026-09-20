"""Capture three disabled TIM3/ADC/DMA probes; never issues a motor command."""
import argparse
from pathlib import Path
import re
from drv_capture import send_line,read_available,verify_off


def validate(text):
    rows=re.findall(r'^ADCTRIGGER result=(\d+) n=32 elapsed_us=(\d+) min=(\d+) max=(\d+) trigger_us=101 ext_sel=3 gate_authority=0$',text,re.M)
    if len(rows)!=3: raise ValueError('three complete trigger results required')
    parsed=[tuple(map(int,r)) for r in rows]
    if any(reason or not 3000<=elapsed<=4000 or not 0<low<=high<4095
           for reason,elapsed,low,high in parsed): raise ValueError('trigger check failed')
    verify_off(text[text.rfind('ADCTRIGGER'):].encode())
    return parsed


if __name__=='__main__':
    ap=argparse.ArgumentParser(description=__doc__)
    ap.add_argument('--out',type=Path)
    ap.add_argument('--input',type=Path)
    args=ap.parse_args()
    if args.input:
        print(validate(args.input.read_text()))
    else:
        if args.out is None or args.out.exists(): ap.error('new --out path required')
        import serial
        raw=b''
        with serial.Serial('COM41',115200,timeout=.05) as port:
            try:
                raw+=read_available(port,.3)
                for command in ['off','p','i']:
                    send_line(port,command);raw+=read_available(port,.25)
                verify_off(raw)
                for _ in range(3):
                    send_line(port,'adctriggercheck');raw+=read_available(port,.3)
            finally:
                for command in ['off','stack','p','i']:
                    send_line(port,command);raw+=read_available(port,.25)
                args.out.write_bytes(raw)
        print(validate(raw.decode(errors='replace')))

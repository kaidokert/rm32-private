"""Read-only SWD snapshot for disabled-board UART diagnosis. Never reads RDR.

Run only without another probe/serial fixture. This is not a safing command.
"""
import argparse
import json
from pathlib import Path
import re
import subprocess

REGISTERS=[('rcc_apbenr1',0x4002103c,1),('rcc_ccipr',0x40021054,1),
           ('gpioc_moder',0x50000800,1),('gpioc_afrh',0x50000824,1),
           ('usart3_control_brr',0x40004800,4),('usart3_isr',0x4000481c,1),
           ('enable_odr',0x50000c14,1),('tim1_bdtr',0x40012c44,1),
           ('tim1_ccr',0x40012c34,3)]


def main():
    ap=argparse.ArgumentParser(description=__doc__)
    ap.add_argument('--out',type=Path,required=True);args=ap.parse_args()
    with args.out.open('x') as dest:
        for name,address,count in REGISTERS:
            cmd=['probe-rs','read','--chip','STM32G071RBTx','--probe',
                 '0483:374b:066CFF343433464757233430','b32',hex(address),str(count)]
            result=subprocess.run(cmd,capture_output=True,text=True,timeout=15)
            tokens=re.findall(r'\b[0-9a-fA-F]{8}\b',result.stdout)
            row=dict(name=name,address=hex(address),count=count,exit_code=result.returncode,
                     stdout=result.stdout,stderr=result.stderr)
            if result.returncode==0 and len(tokens)==count:
                row['values']=[int(t,16) for t in tokens]
            dest.write(json.dumps(row)+'\n');dest.flush()
            if 'values' not in row:raise RuntimeError('snapshot failed; partial evidence retained')
            print(name,' '.join(f'{v:08x}' for v in row['values']))


if __name__=='__main__':main()

"""Disabled atomic semantics/PRIMASK check; does not exercise interrupt contention."""
import argparse
import re
from pathlib import Path
import serial
from drv_capture import send_line,read_available,verify_off


def verify_backend(text, *, single_core=None):
    rows=re.findall(r'^ATOMICBACKEND .*$',text.replace('\r',''),re.M)
    allowed=[f'ATOMICBACKEND single_core={n} explicit_cs_preserved=1' for n in (0,1)]
    if not rows and single_core is None:
        return None
    if len(rows)!=1 or rows[0] not in allowed:
        raise ValueError('missing/invalid atomic backend provenance')
    actual=rows[0]==allowed[1]
    if single_core is not None and actual!=single_core:
        raise ValueError('unexpected atomic backend')
    return actual


def verify(text):
    text=text.replace('\r','')
    if text.count('FINALOFF\n')!=1:
        raise ValueError('missing/duplicate final safing')
    body,final=text.split('FINALOFF\n')
    verify_off(final.encode())
    verify_backend(body,single_core=True)
    rows=re.findall(r'^ATOMICCHECK .*$',body,re.M)
    if rows!=['ATOMICCHECK passed=256 total=256 privileged=1 entry_unmasked=1 restored=1 disabled=1 gate_authority=0']:
        raise ValueError('atomic semantics/privilege/PRIMASK check failed')
    return True


def main():
    ap=argparse.ArgumentParser(description=__doc__)
    ap.add_argument('--out',type=Path,required=True)
    args=ap.parse_args()
    with args.out.open('xb') as raw, serial.Serial('COM41',115200,timeout=.05) as port:
        data=bytearray()
        def command(line):
            send_line(port,line)
            chunk=read_available(port,.4)
            data.extend(chunk);raw.write(chunk);raw.flush()
        try:
            for line in ['off','p','i']:command(line)
            verify_off(bytes(data))
            command('atomiccheck')
        finally:
            data.extend(b'FINALOFF\n');raw.write(b'FINALOFF\n')
            for line in ['off','p','i']:command(line)
    verify(data.decode())
    print('256 atomic/PRIMASK checks passed; privileged, final off verified.')


if __name__=='__main__':main()

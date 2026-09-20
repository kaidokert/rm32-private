"""Idle ENABLE epoch check. Wakes CSA only; no gate commands."""
import argparse
import re
from pathlib import Path
import serial
from drv_capture import send_line, read_available, verify_off


def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--out',type=Path,required=True)
    args=parser.parse_args()
    with args.out.open('xb') as raw:
        data=bytearray()
        with serial.Serial('COM41',115200,timeout=.05) as port:
            def command(line):
                send_line(port,line);chunk=read_available(port,.4)
                data.extend(chunk);raw.write(chunk);raw.flush()
            try:
                for line in ['off','p','i']: command(line)
                verify_off(bytes(data))
                for _ in range(3): command('epochcheck')
            finally:
                data.extend(b'FINALOFF\n');raw.write(b'FINALOFF\n')
                for line in ['off','p','i']: command(line)
        text=data.decode().replace('\r','')
        verify_off(text[text.rfind('FINALOFF\n'):].encode())
        rows=re.findall(r'^EPOCHCHECK passed=5 total=5 repeat_n=256 repeat_sum_us=(\d+) repeat_max_us=(\d+) disabled=1 gates_commanded=0$',text,re.M)
        if len(rows)!=3 or any(int(maximum)>5 or int(total)>256*int(maximum) for total,maximum in rows):
            raise ValueError('epoch identity or initial <=5us write-path budget failed')
        print('Three epoch trials pass; (sum_us,max_us):',rows)


if __name__=='__main__': main()

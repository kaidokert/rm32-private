"""Disabled-board optional meter overhead check; never commands motor power."""
import argparse
import json
import re
from pathlib import Path
import serial
from drv_capture import send_line, read_available, verify_off


def verify(text, *, union=False, scheduling_tail=False, comp_overlap=False):
    text = text.replace('\r', '')
    end = text.rfind('FINALOFF\n')
    if end < 0:
        raise ValueError('missing final safing')
    verify_off(text[end:].encode())
    kinds = re.findall(r'^CPUCHECKTYPE union=([^\r\n]+)', text, re.M)
    overlaps=re.findall(r'^CPUCHECKOVERLAP(?: ([^\r\n]*))?$',text,re.M)
    if (overlaps or comp_overlap) and overlaps!=['enabled=1 nested_exercised=1 extra_clock_reads=0']:
        raise ValueError('CPU overlap provenance failed')
    tails=re.findall(r'^CPUCHECKTAIL(?: ([^\r\n]*))?$',text,re.M)
    if (tails or scheduling_tail) and tails!=['enabled=1 nested_exercised=1 fault_checked=1']:
        raise ValueError('CPU scheduling-tail provenance failed')
    if (kinds and kinds not in [['0'], ['1']]) or (union and kinds != ['1']):
        raise ValueError('wrong/missing CPU overhead protocol')
    checks = re.findall(r'^CPUCHECK mode=(\d+) n=256 sum_us=(\d+) max_us=(\d+) fault=(\d+) software_pairs_only=1 gate_authority=0 disabled=1$', text, re.M)
    if len(checks) != 3 or [int(c[0]) for c in checks] != [0, 1, 2]:
        raise ValueError('missing/duplicate CPU overhead checks')
    result = []
    for mode, total, maximum, fault in (map(int, c) for c in checks):
        if fault or maximum > 10 or not 0 <= total <= maximum * 256:
            raise ValueError('CPU overhead gate failed')
        result.append(dict(mode=mode, mean_pair_us=total / 256, max_pair_us=maximum))
    return result


def main():
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument('--out', required=True, type=Path)
    ap.add_argument('--union', action='store_true')
    ap.add_argument('--scheduling-tail',action='store_true')
    ap.add_argument('--comp-overlap',action='store_true')
    args = ap.parse_args()
    with args.out.open('xb') as raw:
        data = bytearray()
        with serial.Serial('COM41', 115200, timeout=.05) as port:
            def command(line):
                send_line(port, line)
                chunk = read_available(port, .4)
                data.extend(chunk); raw.write(chunk); raw.flush()
            try:
                for line in ['off', 'p', 'i']:
                    command(line)
                verify_off(bytes(data))
                command('cpucheck')
            finally:
                data.extend(b'FINALOFF\n'); raw.write(b'FINALOFF\n')
                for line in ['off', 'p', 'i']:
                    command(line)
        print(json.dumps(verify(data.decode().replace('\r', ''), union=args.union,scheduling_tail=args.scheduling_tail,comp_overlap=args.comp_overlap), indent=2))


if __name__ == '__main__':
    main()

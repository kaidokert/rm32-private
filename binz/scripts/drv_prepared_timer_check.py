"""Disabled-only prepared TIM16 publication; never sends a motor command."""
import argparse
from pathlib import Path
import re
import serial
from drv_capture import send_line, read_available, verify_off


def verify(text):
    body, final = text.replace('\r', '').split('FINALOFF\n')
    verify_off(final.encode())
    rows = re.findall(r'^PREPAREDCHECK trials=(160|192) accepted=(\d+) refused=(\d+) failed=(\d+) '
                      r'max_publish_us=(\d+) origin_span_us=(\d+) deadline_error_us=(\d+) '
                      r'disabled=(\d+) irq_masked=1$', body, re.M)
    if len(rows) != 1:
        raise ValueError('missing/duplicate prepared timer result')
    trials, accepted, refused, failed, cost, span, error, off = map(int, rows[0])
    if (accepted, refused, failed, off) != (trials-128, 128, 0, 1):
        raise ValueError(f'prepared timer semantics failed: {rows[0]}')
    if cost > 8 or span > 2 or error > 3:
        raise ValueError(f'disabled publication timing failed: {rows[0]}')
    if trials==192:
        mapping=re.findall(r'^PREPAREDMAP trials=32 max_mapping_us=(\d+) quantization_bound_ticks=4$',body,re.M)
        if len(mapping)!=1 or int(mapping[0])>8:
            raise ValueError('missing/over-budget clock mapping')
    return dict(accepted=accepted, refused=refused, max_publish_us=cost,
                origin_span_us=span, deadline_error_us=error, outputs_off=True)


def verify_load(text, expected_priority=None):
    body,final=text.replace('\r','').split('FINALOFF\n');verify_off(final.encode())
    rows=re.findall(r'^PREPAREDLOAD trials=(\d+) events=(\d+) failures=(\d+) refusal=(\d+) '
                    r'max_late_us=(\d+) max_dma_us=(\d+) min_feedback=(\d+) disabled=(\d+) '
                    r'diagnostic_com=1 actual_guard_dma=1(?: priority=(0|128))?$',body,re.M)
    if len(rows)!=1:raise ValueError('missing loaded result')
    trials,events,failures,refusal,late,dma,feedback,off=map(int,rows[0][:8])
    priority=int(rows[0][8]) if rows[0][8] else None
    if expected_priority is not None and priority!=expected_priority:
        raise ValueError('missing or wrong diagnostic COM priority')
    if (trials,events,failures,refusal,off)!=(32,32,0,0,1) or not 0<feedback<100:
        raise ValueError(f'loaded semantics failed: {rows[0]}')
    return dict(trials=trials,events=events,max_late_us=late,max_dma_us=dma,
                min_feedback=feedback,timing_qualified=False,outputs_off=True,priority=priority)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--out', type=Path, required=True)
    parser.add_argument('--load',action='store_true')
    parser.add_argument('--priority-zero',action='store_true')
    args = parser.parse_args()
    if args.priority_zero and not args.load:parser.error('--priority-zero needs --load')
    with args.out.open('xb') as raw, serial.Serial('COM41', 115200, timeout=.05) as port:
        data = bytearray()
        def command(line):
            send_line(port, line)
            part = read_available(port, .5)
            data.extend(part);raw.write(part);raw.flush()
        try:
            for line in ['off', 'p', 'i']: command(line)
            verify_off(data)
            command(('preparedload0' if args.priority_zero else 'preparedload') if args.load else 'preparedcheck')
        finally:
            raw.write(b'FINALOFF\n');data.extend(b'FINALOFF\n')
            for line in ['off', 'p', 'i']: command(line)
    print(verify_load(data.decode(),0 if args.priority_zero else 128) if args.load else verify(data.decode()))


if __name__ == '__main__': main()

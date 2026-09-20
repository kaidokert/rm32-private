"""ENABLE-low role/counter preflight; never commands driver ENABLE high."""
import argparse
import json
import re
from pathlib import Path
import serial
from drv_capture import send_line, read_available, verify_off


def verify_mode(text, required=False, carrier_hz=None):
    markers = re.findall(r'^PWMROLES .*$', text.replace('\r', ''), re.M)
    expected = 'PWMROLES bemf_only=1 equal_ccr=1 initial_ug=1 per_com_ug=0 forced_startup_unchanged=1 carrier_hz=10000'
    candidates = [[expected.replace('10000',str(hz))] for hz in (10000,20000,24006,32000,40000)]
    if (required or markers or carrier_hz is not None) and markers not in candidates:
        raise ValueError('missing/unknown/duplicate PWM role provenance')
    if carrier_hz is not None and (carrier_hz not in (10000,20000,24006,32000,40000) or markers != [expected.replace('10000',str(carrier_hz))]):
        raise ValueError('PWM carrier differs from requested experiment')
    return bool(markers)


def verify(text, *, period_ticks=None, duty=None):
    text = text.replace('\r', '')
    if text.count('FINALOFF\n') != 1:
        raise ValueError('missing/duplicate final safing')
    body, final = text.split('FINALOFF\n')
    verify_off(final.encode())
    headers = re.findall(r'^ROLECHECKPERIOD .*$', body, re.M)
    period = 6400
    if headers:
        match = re.fullmatch(r'ROLECHECKPERIOD ticks=(6400|3200|2666) timer_hz=64000000 restored_ticks=6400',headers[0]) if len(headers)==1 else None
        if not match:
            raise ValueError('unknown/duplicate carrier period')
        period = int(match[1])
    if period_ticks is not None and (not headers or period != period_ticks):
        raise ValueError('carrier period differs from requested diagnostic')
    duty_rows=re.findall(r'^ROLECHECKDUTY .*$',body,re.M)
    if duty_rows or duty is not None:
        match=re.fullmatch(r'ROLECHECKDUTY tenths=(\d+) compare=(\d+) all_three_checked=1',duty_rows[0]) if len(duty_rows)==1 else None
        if not match:raise ValueError('missing/malformed role duty readback')
        applied,compare=map(int,match.groups())
        if not 40<=applied<=100 or compare!=period*applied//1000 or (duty is not None and applied!=duty):
            raise ValueError('wrong role duty/compare readback')
    parts = re.findall(r'^ROLECHECKPART .*$', body, re.M)
    rows = []
    for line in parts:
        match = re.fullmatch(r'ROLECHECKPART step=(\d+) flags=(\d+) elapsed_us=(\d+) delta_ticks=(\d+) error_mod=(\d+)', line)
        if not match:
            raise ValueError('malformed role row')
        step, flags, elapsed, delta, error = map(int, match.groups())
        if flags != 127 or not 0 <= elapsed < min(70,period//64-3) or not 0 <= delta < period:
            raise ValueError('role/counter gate failed')
        if error != (delta - elapsed * 64) % period or not (error <= 192 or error >= period-192):
            raise ValueError('counter continuity gate failed')
        rows.append(dict(step=step, elapsed_us=elapsed, delta_ticks=delta, error_mod=error))
    if [r['step'] for r in rows] != list(range(1, 7)):
        raise ValueError('missing/duplicate/out-of-order sectors')
    summaries = re.findall(r'^ROLECHECK .*$', body, re.M)
    if summaries != ['ROLECHECK passed=6 expected=6 restored=1 disabled=1 motor_authority=0']:
        raise ValueError('role safing/restore failed')
    if body.index(summaries[0]) < body.rfind('ROLECHECKPART') or 'ROLECHECK' in final:
        raise ValueError('role records outside diagnostic window')
    return rows


def main():
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument('--out', type=Path, required=True)
    ap.add_argument('--period-ticks', type=int, choices=[6400,3200,2666])
    ap.add_argument('--duty',type=int,choices=range(40,101))
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
                command('rolecheck' if args.duty is None else f'roledu{args.duty}')
            finally:
                data.extend(b'FINALOFF\n'); raw.write(b'FINALOFF\n')
                for line in ['off', 'p', 'i']:
                    command(line)
        print(json.dumps(verify(data.decode(),period_ticks=args.period_ticks,duty=args.duty), indent=2))


if __name__ == '__main__':
    main()

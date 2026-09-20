"""Disabled TIM1 live-duty preload check; never enables the DRV."""
import argparse
import re
from pathlib import Path
import serial

from drv_capture import read_available, send_line, verify_off


def verify(text: str):
    text = text.replace('\r', '')
    if text.count('FINALOFF\n') != 1:
        raise ValueError('missing unique finaloff boundary')
    body, final = text.split('FINALOFF\n')
    verify_off(final.encode())
    rows = re.findall(
        r'^LIVEDUTYCHECK passed=(\d+) expected=24 max_us=(\d+) '
        r'disabled=(\d+) shadow_readback=0 motor_authority=0$', body, re.M
    )
    if len(rows) != 1:
        raise ValueError(f'missing unique live-duty check: {rows}')
    passed, maximum, disabled = map(int, rows[0])
    if passed != 24 or disabled != 1 or maximum > 20:
        raise ValueError(f'live-duty hardware gate failed: {rows[0]}')
    return dict(transactions=passed, max_us=maximum, outputs_off=True)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--out', type=Path, required=True)
    args = parser.parse_args()
    data = bytearray()
    with args.out.open('xb') as raw, serial.Serial('COM41', 115200, timeout=.05) as port:
        # A just-reset board can still be completing clock/UART setup when the
        # host opens COM41. Drain/wait before the first safety transaction.
        read_available(port, .5)
        def command(line):
            send_line(port, line)
            chunk = read_available(port, .6)
            data.extend(chunk); raw.write(chunk); raw.flush()
        try:
            for line in ('off', 'p', 'i'):
                command(line)
            verify_off(data)
            command('livedutycheck')
        finally:
            data.extend(b'FINALOFF\n'); raw.write(b'FINALOFF\n')
            for line in ('off', 'p', 'i'):
                command(line)
    print(verify(data.decode()))


if __name__ == '__main__':
    main()

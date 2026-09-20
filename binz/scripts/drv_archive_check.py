"""Disabled archive routing check; retains raw output even on failure."""
import argparse
from pathlib import Path
import serial
from drv_capture import send_line, read_available, verify_off


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--out', type=Path, required=True)
    args = parser.parse_args()
    with args.out.open('xb') as raw:
        data = bytearray()
        with serial.Serial('COM41', 115200, timeout=.05) as port:
            def command(line):
                send_line(port, line)
                chunk = read_available(port, .4)
                data.extend(chunk)
                raw.write(chunk)
                raw.flush()
            try:
                for line in ['off', 'p', 'i']:
                    command(line)
                verify_off(bytes(data))
                for _ in range(3):
                    command('archivecheck')
            finally:
                data.extend(b'FINALOFF\n')
                raw.write(b'FINALOFF\n')
                for line in ['off', 'p', 'i']:
                    command(line)
        text = data.decode().replace('\r', '')
        verify_off(text[text.rfind('FINALOFF\n'):].encode())
        assert text.count('ARCHIVECHECK passed=3 total=3 gate_authority=0 disabled=1\n') == 3, text
        print('Archive routing/refusal: 3/3 checks on each of 3 trials; final off verified.')


if __name__ == '__main__':
    main()

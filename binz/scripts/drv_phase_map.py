"""Replay or capture UNLOADED single-input identity tests. Never run with motor connected."""
import argparse
import pathlib
import re

from drv_capture import read_available, send_line


def analyze(text):
    blocks = re.findall(r'MAP input=(\d) pulse_us=(\d+) fault=(\d).*?\n(.*?)(?=MAP input=|\Z)', text, re.S)
    seen = {i: [] for i in range(6)}
    for selected, duration, fault, body in blocks:
        selected = int(selected)
        rows = re.findall(r'^MAP stage=(\d) (.+)$', body, re.M)
        assert [int(s) for s, _ in rows] == [0, 1, 2], 'missing or unordered stages'
        rows = [list(map(int, row.split())) for _, row in rows]
        assert all(len(row) == 10 for row in rows), 'record width'
        assert fault == '0' and 0 < int(duration) < 2000, 'fault or pulse duration'
        assert [r[8] for r in rows] == [0, 1 << selected, 0], 'MCU gate readback mismatch'
        assert all(r[7] & 1 for r in rows), 'sampled fault'
        bits = (rows[1][7] >> 1) & 7
        # Neutral > phase: H should uniquely clear its bit; L uniquely set it.
        unique = (bits ^ 7) if selected < 3 else bits
        assert unique in (1, 2, 4), 'ambiguous comparator response'
        phase = (1, 2, 4).index(unique)
        # The analog C channel independently checks comparator C polarity.
        delta = rows[1][3] - rows[1][4]
        assert (delta < 0) == bool(bits & 4), 'C ADC/comparator polarity conflict'
        seen[selected].append(phase)
    for i in range(6):
        assert len(seen[i]) >= 2 and len(set(seen[i])) == 1, f'input {i}: insufficient repeatability'
    high = [seen[i][0] for i in range(3)]
    low = [seen[i][0] for i in range(3, 6)]
    assert sorted(high) == sorted(low) == [0, 1, 2], 'not a phase permutation'
    for i, name in enumerate(['AH', 'BH', 'CH', 'AL', 'BL', 'CL']):
        print(f'{name} -> VSEN {"ABC"[seen[i][0]]} ({len(seen[i])} repeats)')
    print('paired=', high == low, 'nominal_order=', high == low == [0, 1, 2])
    print('ISEN identity NOT established by this unloaded test')


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--infile', type=pathlib.Path)
    parser.add_argument('--motor-disconnected', action='store_true')
    parser.add_argument('--out', type=pathlib.Path)
    args = parser.parse_args()
    if args.infile:
        text = args.infile.read_text()
    else:
        if not args.motor_disconnected or not args.out:
            parser.error('live requires --motor-disconnected and --out')
        import serial
        text = ''
        with serial.Serial('COM41', 115200, timeout=0.05) as port:
            try:
                for command in ['off'] + [f'map{i}' for _ in range(2) for i in range(6)] + ['p', 'i']:
                    send_line(port, command)
                    part = read_available(port, 0.3).decode('ascii')
                    text += part
                    if command.startswith('map'):
                        assert 'fault=0' in part and 'en=0 moe=0' in part, 'diagnostic did not finish safely'
            finally:
                send_line(port, 'off')
                text += read_available(port, 0.2).decode('ascii')
                args.out.write_text(text)
    analyze(text)


if __name__ == '__main__':
    main()

"""Decode CRC-protected accepted-event acquisition; not a lock classifier."""
import argparse
import base64
import re
import struct
import zlib
from pathlib import Path


def decode(text, *, _prefix_only=False):
    header = re.search(r'^ACCEPTLOG n=(\d+) drop=(\d+) ', text, re.M)
    if not header:
        raise ValueError('missing ACCEPTLOG')
    count, dropped = map(int, header.groups())
    if dropped and not _prefix_only:
        raise ValueError('accepted-event log overflow; incomplete stream')
    result = []
    for line in text.splitlines():
        if not line.startswith('A85 '):
            continue
        raw = base64.a85decode(line[4:])
        if len(raw) != 12 or zlib.crc32(raw[:8]) != int.from_bytes(raw[8:], 'little'):
            raise ValueError('accepted-event length/CRC')
        low, high, step, interval = struct.unpack('<4H', raw[:8])
        if not 1 <= step <= 6:
            raise ValueError('accepted-event sector')
        result.append(dict(us=low+(high << 16), step=step, reference_interval_ticks=interval))
    if len(result) != count:
        raise ValueError('accepted-event count')
    return result


def decode_windows(text):
    """Return explicit prefix/suffix windows, never an invented continuous trace."""
    prefix = decode(text, _prefix_only=True)
    header = re.search(r'^ACCEPTTAIL n=(\d+) skipped=(\d+) total=(\d+) ', text, re.M)
    if not header:
        # Legacy captures must still be complete; no silent overflow tolerance.
        return dict(prefix=decode(text), tail=[], skipped=0, total=len(prefix))
    count, skipped, total = map(int, header.groups())
    drop = int(re.search(r'^ACCEPTLOG n=\d+ drop=(\d+) ', text, re.M)[1])
    if (len(prefix) != min(total, 32) or total != len(prefix)+drop or
            count != min(drop, 32) or skipped != max(drop-32, 0)):
        raise ValueError('accepted window accounting')
    # Reuse the exact CRC/length/sector decoder for the suffix wire records.
    tail_text = f'ACCEPTLOG n={count} drop=0 fields=tail\n' + '\n'.join(
        'A85 '+line[4:] for line in text.splitlines() if line.startswith('T85 '))
    tail = decode(tail_text)
    for window in (prefix, tail):
        if any(b['us'] <= a['us'] or b['step'] != a['step'] % 6+1
               for a, b in zip(window, window[1:])):
            raise ValueError('accepted window order')
    if prefix and tail:
        a, b = prefix[-1], tail[0]
        if b['us'] <= a['us'] or b['step'] != (a['step']-1+skipped+1) % 6+1:
            raise ValueError('accepted window boundary')
    return dict(prefix=prefix, tail=tail, skipped=skipped, total=total)


def decode_first_segment(text):
    """Separate frozen windows; never merge with a subsequent live segment."""
    headers=re.findall(r'^FIRSTSEG ([^\r\n]+)',text,re.M)
    if len(headers)!=1: raise ValueError('missing/duplicate FIRSTSEG')
    h={k:int(v) for k,v in re.findall(r'(\w+)=(\d+)',headers[0])}
    if not {'fault','events','prefix','tail','skipped','end_us'}<=h.keys():
        raise ValueError('incomplete FIRSTSEG')
    converted=[f"ACCEPTLOG n={h['prefix']} drop={h['events']-h['prefix']} fields=frozen",
               f"ACCEPTTAIL n={h['tail']} skipped={h['skipped']} total={h['events']} fields=frozen"]
    for line in text.splitlines():
        if line.startswith('P185 '): converted.append('A85 '+line[5:])
        if line.startswith('T185 '): converted.append('T85 '+line[5:])
    windows=decode_windows('\n'.join(converted))
    if any(e['us']>h['end_us'] for e in windows['prefix']+windows['tail']):
        raise ValueError('archived event after stopped observation')
    return dict(header=h,**windows)


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('capture', type=Path)
    parser.add_argument('--windows', action='store_true', help='decode explicit start/end windows with omitted count')
    args = parser.parse_args()
    if args.windows:
        print(decode_windows(args.capture.read_text()))
    else:
        for event in decode(args.capture.read_text()):
            print(event)

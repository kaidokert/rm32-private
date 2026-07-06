"""Shared MAGPIE frame definitions/parser for the motor_tester2 stream.

Frame layout v3 (26 bytes, little-endian):
    [0]  0x5A  sync
    [1]  0xA5  sync
    [2]  seq (u8, wraps)
    [3]  bit7 = zc_found, bits0-3 = sector (0..5)
    [4:8]   window start, 10 µs ticks (u32)
    [8:10]  window length, 10 µs ticks (u16)
    [10:12] first-valid-ZC offset from window start, µs (u16, 0xFFFF = none)
    [12:14] raw edge count (u16)
    [14:16] gate-surviving edge count (u16)
    [16:18] window current min, raw 12-bit ADC counts (u16)
    [18:20] window current max (u16)
    [20:22] window current mean (u16)
    [22:24] OWL first persistence-QUALIFIED ZC offset, µs (u16, 0xFFFF = none)
    [24:26] OWL prediction error, µs (i16; INT16_MIN = no prediction)

Current conversion: INA180B1 (20 V/V) x 1.5 mOhm shunt = 30 mV/A,
VDDA nominal 3.3 V, 12-bit.
"""

FRAME_LEN = 26
SYNC0 = 0x5A
SYNC1 = 0xA5
PRED_NONE = -32768


def raw_to_ma(raw: int, vdda_mv: int = 3300) -> float:
    """12-bit ADC counts -> milliamps (nominal VDDA)."""
    return raw * vdda_mv / 4095 / 30 * 1000


def parse_frames(buf: bytes):
    """Scan for framed records; tolerates interleaved ASCII output."""
    frames = []
    i = 0
    while i + FRAME_LEN <= len(buf):
        if buf[i] == SYNC0 and buf[i + 1] == SYNC1 and (buf[i + 3] & 0x0F) < 6:
            f = buf[i : i + FRAME_LEN]
            frames.append(
                dict(
                    seq=f[2],
                    zc_found=bool(f[3] & 0x80),
                    sector=f[3] & 0x0F,
                    start=int.from_bytes(f[4:8], "little"),
                    len_us=int.from_bytes(f[8:10], "little") * 10,
                    zc_off_us=int.from_bytes(f[10:12], "little"),
                    raw=int.from_bytes(f[12:14], "little"),
                    valid=int.from_bytes(f[14:16], "little"),
                    i_min=int.from_bytes(f[16:18], "little"),
                    i_max=int.from_bytes(f[18:20], "little"),
                    i_avg=int.from_bytes(f[20:22], "little"),
                    qzc_off_us=int.from_bytes(f[22:24], "little"),
                    pred_err_us=int.from_bytes(f[24:26], "little", signed=True),
                )
            )
            i += FRAME_LEN
        else:
            i += 1
    return frames


def seq_gaps(frames) -> int:
    return sum(1 for a, b in zip(frames, frames[1:]) if (a["seq"] + 1) % 256 != b["seq"])

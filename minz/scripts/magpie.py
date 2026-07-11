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

FRAME_LEN = 26  # v3
FRAME_LEN_V4 = 28  # v4: + vbat_raw u16 (window-min supply voltage)
SYNC0 = 0x5A
SYNC1 = 0xA5  # v3
SYNC1_V4 = 0xA6
PRED_NONE = -32768

VBAT_UV_PER_COUNT = 7507  # divider-scaled: raw 1080 ≈ 8.107 V


def vbat_raw_to_v(raw: int) -> float:
    return raw * VBAT_UV_PER_COUNT / 1e6


def raw_to_ma(raw: int, vdda_mv: int = 3300) -> float:
    """12-bit ADC counts -> milliamps (nominal VDDA)."""
    return raw * vdda_mv / 4095 / 30 * 1000


def parse_frames(buf: bytes):
    """Scan for framed records; tolerates interleaved ASCII output.

    The frame has no checksum, so a sync-pattern hit inside binary
    garbage (e.g. a capture that opens mid-stream on a partial frame)
    can mis-lock and decode noise. Field sanity checks reject those:
    sector 0-5, plausible window length, 12-bit current fields with
    min <= avg <= max, and valid <= raw edge counts.
    """
    frames = []
    i = 0
    while i + FRAME_LEN <= len(buf):
        is_v4 = buf[i] == SYNC0 and buf[i + 1] == SYNC1_V4
        if is_v4 and i + FRAME_LEN_V4 > len(buf):
            break
        if (buf[i] == SYNC0 and buf[i + 1] == SYNC1 and (buf[i + 3] & 0x0F) < 6) or (
            is_v4 and (buf[i + 3] & 0x0F) < 6
        ):
            flen = FRAME_LEN_V4 if is_v4 else FRAME_LEN
            f = buf[i : i + flen]
            len_10us = int.from_bytes(f[8:10], "little")
            raw = int.from_bytes(f[12:14], "little")
            valid = int.from_bytes(f[14:16], "little")
            i_min = int.from_bytes(f[16:18], "little")
            i_max = int.from_bytes(f[18:20], "little")
            i_avg = int.from_bytes(f[20:22], "little")
            sane = (
                0 < len_10us < 3000  # 10 µs .. 30 ms window
                and valid <= raw
                and i_min <= 0x0FFF
                and i_max <= 0x0FFF
                and i_min <= i_avg <= i_max
            )
            if not sane:
                i += 1
                continue
            frames.append(
                dict(
                    seq=f[2],
                    zc_found=bool(f[3] & 0x80),
                    sector=f[3] & 0x0F,
                    start=int.from_bytes(f[4:8], "little"),
                    len_us=len_10us * 10,
                    zc_off_us=int.from_bytes(f[10:12], "little"),
                    raw=raw,
                    valid=valid,
                    i_min=i_min,
                    i_max=i_max,
                    i_avg=i_avg,
                    qzc_off_us=int.from_bytes(f[22:24], "little"),
                    pred_err_us=int.from_bytes(f[24:26], "little", signed=True),
                    # v4: worst supply voltage within/since-last-streamed
                    # window (48 kHz pump, firmware-aggregated min).
                    vbat_raw=int.from_bytes(f[26:28], "little") if is_v4 else None,
                )
            )
            i += flen
        else:
            i += 1
    return frames


def seq_gaps(frames) -> int:
    return sum(1 for a, b in zip(frames, frames[1:]) if (a["seq"] + 1) % 256 != b["seq"])


def parse_cdump(text: str):
    """Parse a WAXWING `cdump:` burst (Ascii85, 4 u16 channels/frame).

    Returns (frames, sample_hz) where each frame is a dict with a, b,
    i (raw 12-bit), sector, comp (wrap-sampled COMP2 bit).
    """
    import base64
    import struct

    lines = text.splitlines()
    hdr_i = next(i for i, l in enumerate(lines) if l.lstrip().startswith("cdump:"))
    sample_hz = float(lines[hdr_i].split("sample_hz=")[1].split()[0])
    body = []
    for l in lines[hdr_i + 1 :]:
        if l.strip() == "end":
            break
        body.append(l.strip())
    raw = base64.a85decode("".join(body).encode("ascii"))
    vals = struct.unpack(f"<{len(raw) // 2}H", raw[: len(raw) // 2 * 2])
    frames = []
    for k in range(len(vals) // 4):
        a, b, i, st = vals[4 * k : 4 * k + 4]
        frames.append(dict(a=a, b=b, i=i, sector=(st >> 1) & 7, comp=st & 1))
    return frames, sample_hz

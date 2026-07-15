//! MAGPIE window-record wire format (v5) — encode AND decode, so the
//! frame layout is round-trip tested on the host instead of debugged
//! across a UART. The decode carries the same field-sanity rules the
//! host parser uses (the frame has no checksum; a capture opening
//! mid-stream can sync-mislock on binary garbage and decode noise —
//! 1,400 % jitter and 3 A phantom currents in one render).

pub const SYNC0: u8 = 0x5A;
/// v5 sync (v4 was 0xA6 with len in 10 µs ticks; v3 was 0xA5 at 26
/// bytes). v5 keeps the 28-byte layout but carries the window length
/// in MICROSECONDS — the 10 µs tick was 11 % quantization at ~92 µs
/// high-speed windows and painted false ±300 Hz "oscillation" bands on
/// every dropout plot (2026-07-14 incident). u16 µs = 65 ms range.
pub const SYNC1_V5: u8 = 0xA7;
pub const FRAME_LEN_V5: usize = 28;
/// Back-compat alias (same frame size since v4).
pub const FRAME_LEN_V4: usize = FRAME_LEN_V5;
pub const PRED_NONE: i16 = i16::MIN;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WindowRec {
    pub start_10us: u32,
    pub len_us: u16,
    pub zc_off_us: u16,
    pub raw: u16,
    pub valid: u16,
    pub i_min: u16,
    pub i_max: u16,
    pub i_avg: u16,
    pub qzc_off_us: u16,
    pub pred_err_us: i16,
    /// v4: minimum vbat raw within/since the last streamed window
    /// (48 kHz pump, firmware-aggregated so decimation gaps fold in).
    pub vbat_raw: u16,
    pub sector: u8,
    pub seq: u8,
}

impl WindowRec {
    pub fn encode(&self) -> [u8; FRAME_LEN_V5] {
        let mut f = [0u8; FRAME_LEN_V5];
        f[0] = SYNC0;
        f[1] = SYNC1_V5;
        f[2] = self.seq;
        f[3] = (self.sector & 0x0F) | if self.zc_off_us != 0xFFFF { 0x80 } else { 0 };
        f[4..8].copy_from_slice(&self.start_10us.to_le_bytes());
        f[8..10].copy_from_slice(&self.len_us.to_le_bytes());
        f[10..12].copy_from_slice(&self.zc_off_us.to_le_bytes());
        f[12..14].copy_from_slice(&self.raw.to_le_bytes());
        f[14..16].copy_from_slice(&self.valid.to_le_bytes());
        f[16..18].copy_from_slice(&self.i_min.to_le_bytes());
        f[18..20].copy_from_slice(&self.i_max.to_le_bytes());
        f[20..22].copy_from_slice(&self.i_avg.to_le_bytes());
        f[22..24].copy_from_slice(&self.qzc_off_us.to_le_bytes());
        f[24..26].copy_from_slice(&self.pred_err_us.to_le_bytes());
        f[26..28].copy_from_slice(&self.vbat_raw.to_le_bytes());
        f
    }

    /// Decode one v5 frame with the host parser's sanity rules.
    /// Returns `None` on sync/sanity failure (caller advances 1 byte
    /// and rescans — mirroring `scripts/magpie.py`).
    pub fn decode(f: &[u8]) -> Option<Self> {
        if f.len() < FRAME_LEN_V5 || f[0] != SYNC0 || f[1] != SYNC1_V5 {
            return None;
        }
        let sector = f[3] & 0x0F;
        if sector > 5 {
            return None;
        }
        let u16le = |a: usize| u16::from_le_bytes([f[a], f[a + 1]]);
        let len_us = u16le(8);
        let raw = u16le(12);
        let valid = u16le(14);
        let (i_min, i_max, i_avg) = (u16le(16), u16le(18), u16le(20));
        // A ZC offset can never exceed the window length (+10 µs for
        // the len truncation). Catches sync-mislocked garbage whose
        // offset fields land on stream bytes — one such frame put
        // 0x5A04 in qzc_off (the sync bytes themselves) and blew a
        // map's σ statistic 25× (2026-07-11).
        let off_ok = |off: u16| off == 0xFFFF || off as u32 <= len_us as u32 + 1;
        let sane = len_us > 0
            && len_us < 30_000
            && valid <= raw
            && i_min <= 0x0FFF
            && i_max <= 0x0FFF
            && i_min <= i_avg
            && i_avg <= i_max
            && off_ok(u16le(10))
            && off_ok(u16le(22));
        if !sane {
            return None;
        }
        Some(Self {
            start_10us: u32::from_le_bytes([f[4], f[5], f[6], f[7]]),
            len_us,
            zc_off_us: u16le(10),
            raw,
            valid,
            i_min,
            i_max,
            i_avg,
            qzc_off_us: u16le(22),
            pred_err_us: i16::from_le_bytes([f[24], f[25]]),
            vbat_raw: u16le(26),
            sector,
            seq: f[2],
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> WindowRec {
        WindowRec {
            start_10us: 123_456,
            len_us: 650,
            zc_off_us: 210,
            raw: 55,
            valid: 33,
            i_min: 20,
            i_max: 40,
            i_avg: 30,
            qzc_off_us: 330,
            pred_err_us: -17,
            vbat_raw: 1080,
            sector: 4,
            seq: 42,
        }
    }

    #[test]
    fn roundtrip() {
        let r = sample();
        assert_eq!(WindowRec::decode(&r.encode()), Some(r));
    }

    #[test]
    fn zc_found_flag_bit7() {
        let mut r = sample();
        assert_eq!(r.encode()[3] & 0x80, 0x80);
        r.zc_off_us = 0xFFFF;
        assert_eq!(r.encode()[3] & 0x80, 0);
    }

    #[test]
    fn regression_sync_mislock_rejected() {
        // Garbage that happens to carry the sync pair must be
        // rejected by field sanity — the checksum-less frame's only
        // defense (phantom 3 A currents / 1,400 % jitter incident).
        let mut junk = [0xEEu8; FRAME_LEN_V4];
        junk[0] = SYNC0;
        junk[1] = SYNC1_V5;
        junk[3] = 0x03; // plausible sector
        // len = 0xEEEE > 30 000 → insane.
        assert_eq!(WindowRec::decode(&junk), None);
    }

    #[test]
    fn regression_offset_beyond_window_rejected_2026_07_11() {
        // The bench artifact: a mislocked frame decoded with
        // qzc_off = 0x5A04 (23,044 µs — the next frame's sync bytes)
        // on a 190 µs window, blowing the map's qZC σ from 8 % to
        // 205 %. Offsets must fit the window (or be the 0xFFFF
        // "none" sentinel).
        let mut r = sample();
        r.qzc_off_us = 0x5A04;
        assert_eq!(WindowRec::decode(&r.encode()), None);
        let mut r = sample();
        r.zc_off_us = 0x5A04;
        assert_eq!(WindowRec::decode(&r.encode()), None);
        // Sentinel and boundary (len 650 µs, +1 µs truncation margin)
        // still pass.
        let mut r = sample();
        r.qzc_off_us = 0xFFFF;
        r.zc_off_us = 651;
        assert!(WindowRec::decode(&r.encode()).is_some());
    }

    #[test]
    fn current_ordering_enforced() {
        let mut r = sample();
        r.i_avg = 50; // above i_max
        assert_eq!(WindowRec::decode(&r.encode()), None);
    }

    #[test]
    fn valid_cannot_exceed_raw() {
        let mut r = sample();
        r.valid = 100;
        r.raw = 10;
        assert_eq!(WindowRec::decode(&r.encode()), None);
    }

    #[test]
    fn sector_range_enforced() {
        let mut f = sample().encode();
        f[3] = 0x06; // sector 6
        assert_eq!(WindowRec::decode(&f), None);
    }
}

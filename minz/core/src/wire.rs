//! MAGPIE window-record wire format (v4) — encode AND decode, so the
//! frame layout is round-trip tested on the host instead of debugged
//! across a UART. The decode carries the same field-sanity rules the
//! host parser uses (the frame has no checksum; a capture opening
//! mid-stream can sync-mislock on binary garbage and decode noise —
//! 1,400 % jitter and 3 A phantom currents in one render).

pub const SYNC0: u8 = 0x5A;
/// v4 sync (v3 was 0xA5 at 26 bytes).
pub const SYNC1_V4: u8 = 0xA6;
pub const FRAME_LEN_V4: usize = 28;
pub const PRED_NONE: i16 = i16::MIN;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WindowRec {
    pub start_10us: u32,
    pub len_10us: u16,
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
    pub fn encode(&self) -> [u8; FRAME_LEN_V4] {
        let mut f = [0u8; FRAME_LEN_V4];
        f[0] = SYNC0;
        f[1] = SYNC1_V4;
        f[2] = self.seq;
        f[3] = (self.sector & 0x0F) | if self.zc_off_us != 0xFFFF { 0x80 } else { 0 };
        f[4..8].copy_from_slice(&self.start_10us.to_le_bytes());
        f[8..10].copy_from_slice(&self.len_10us.to_le_bytes());
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

    /// Decode one v4 frame with the host parser's sanity rules.
    /// Returns `None` on sync/sanity failure (caller advances 1 byte
    /// and rescans — mirroring `scripts/magpie.py`).
    pub fn decode(f: &[u8]) -> Option<Self> {
        if f.len() < FRAME_LEN_V4 || f[0] != SYNC0 || f[1] != SYNC1_V4 {
            return None;
        }
        let sector = f[3] & 0x0F;
        if sector > 5 {
            return None;
        }
        let u16le = |a: usize| u16::from_le_bytes([f[a], f[a + 1]]);
        let len_10us = u16le(8);
        let raw = u16le(12);
        let valid = u16le(14);
        let (i_min, i_max, i_avg) = (u16le(16), u16le(18), u16le(20));
        let sane = len_10us > 0
            && len_10us < 3000
            && valid <= raw
            && i_min <= 0x0FFF
            && i_max <= 0x0FFF
            && i_min <= i_avg
            && i_avg <= i_max;
        if !sane {
            return None;
        }
        Some(Self {
            start_10us: u32::from_le_bytes([f[4], f[5], f[6], f[7]]),
            len_10us,
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
            len_10us: 65,
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
        junk[1] = SYNC1_V4;
        junk[3] = 0x03; // plausible sector
        // len = 0xEEEE > 3000 → insane.
        assert_eq!(WindowRec::decode(&junk), None);
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

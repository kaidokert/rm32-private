//! Bench dump serializers — the byte-exact formatters behind the
//! `j` (WAXWING cdump), `l` (aligned PWM-sample dump), `e`/`E`
//! (edge-buffer dump), and desync black-box dumps. Data comes in as
//! slices/closures, bytes go out through a `sink` — so the framing
//! host scripts depend on (`waxwing.py`, rinz cdump readers) is
//! locked by tests instead of debugged across a UART.

use core::fmt::Write;

/// Adapter: `write!` formatting straight into a byte sink.
struct SinkFmt<'a, S: FnMut(&[u8])> {
    sink: &'a mut S,
}

impl<S: FnMut(&[u8])> Write for SinkFmt<'_, S> {
    fn write_str(&mut self, s: &str) -> core::fmt::Result {
        (self.sink)(s.as_bytes());
        Ok(())
    }
}

/// WAXWING cdump body: `n_frames` 4-word frames, each little-endian
/// packed to 8 bytes = two Ascii85 groups = 10 chars; 8 frames per
/// 80-char line, CRLF line ends, `end` terminator. `frame(k)` is
/// called once per frame in dump order (caller handles ring
/// alignment). Header line stays with the caller (it interpolates
/// live settings).
pub fn wax_cdump<F, S>(n_frames: usize, mut frame: F, mut sink: S)
where
    F: FnMut(usize) -> [u16; 4],
    S: FnMut(&[u8]),
{
    let mut line = [0u8; 82];
    let mut pos = 0usize;
    for k in 0..n_frames {
        let w = frame(k);
        let mut bytes = [0u8; 8];
        for (n, v) in w.iter().enumerate() {
            bytes[2 * n..2 * n + 2].copy_from_slice(&v.to_le_bytes());
        }
        line[pos..pos + 5]
            .copy_from_slice(&crate::a85::encode_group(bytes[0..4].try_into().unwrap()));
        line[pos + 5..pos + 10]
            .copy_from_slice(&crate::a85::encode_group(bytes[4..8].try_into().unwrap()));
        pos += 10;
        if pos >= 80 {
            line[pos] = b'\r';
            line[pos + 1] = b'\n';
            sink(&line[..pos + 2]);
            pos = 0;
        }
    }
    if pos > 0 {
        line[pos] = b'\r';
        line[pos + 1] = b'\n';
        sink(&line[..pos + 2]);
    }
    sink(b"end\r\n");
}

/// Locate the last two complete electrical revs in the PWM-sample
/// ring (byte encoding: bit 0 = COMP value, bits 1..=3 = sector).
/// Walks backwards from `head` for three 5→0 sector transitions;
/// three transitions enclose exactly two revs. Returns
/// `Ok((start_index, length))` or `Err(transitions_found)`.
/// `snap.len()` must be a power of two.
pub fn pwm_rev_span(snap: &[u8], head: usize) -> Result<(usize, usize), usize> {
    let mask = snap.len() - 1;
    let mut rev_starts: [usize; 3] = [0; 3];
    let mut found = 0usize;
    let mut newer_sec: u8 = (snap[(head + snap.len() - 1) & mask] >> 1) & 0x07;
    for i in 2..=snap.len() {
        let cur_idx = (head + snap.len() - i) & mask;
        let cur_sec = (snap[cur_idx] >> 1) & 0x07;
        if cur_sec == 5 && newer_sec == 0 {
            rev_starts[found] = (cur_idx + 1) & mask;
            found += 1;
            if found == 3 {
                break;
            }
        }
        newer_sec = cur_sec;
    }
    if found >= 3 {
        // rev_starts[0] = newest 5→0 transition, [2] = oldest. Span
        // [rev_starts[2], rev_starts[0]) is exactly 2 complete revs.
        let r_start = rev_starts[2];
        let length = (rev_starts[0] + snap.len() - r_start) & mask;
        Ok((r_start, length))
    } else {
        Err(found)
    }
}

/// Aligned PWM-sample dump body (`l` key): one `[rev N sec S]: `
/// chunk per sector entry, `.`/`#` for COMP 0/1, and a `*`/`o`
/// textbook-ZC marker at the midpoint of the observed phase's float
/// sectors (`float_s0`/`float_s1`). `(r_start, length)` comes from
/// [`pwm_rev_span`].
pub fn pwm_sector_dump<S: FnMut(&[u8])>(
    snap: &[u8],
    r_start: usize,
    length: usize,
    float_s0: u8,
    float_s1: u8,
    mut sink: S,
) {
    let mask = snap.len() - 1;
    let mut line = [0u8; 200];
    let mut rev_label = 0u8;
    let mut chunks_seen = 0u8;
    let mut chunk_sec: u8 = 0xFF;
    let mut line_len = 0usize;
    let flush_chunk = |line: &mut [u8; 200], line_len: &mut usize, sec: u8, sink: &mut S| {
        if *line_len > 0 {
            if (sec == float_s0 || sec == float_s1) && *line_len >= 2 {
                let mid = *line_len / 2;
                line[mid] = if line[mid] == b'#' { b'*' } else { b'o' };
            }
            line[*line_len] = b'\r';
            line[*line_len + 1] = b'\n';
            sink(&line[..*line_len + 2]);
            *line_len = 0;
        }
    };
    for i in 0..length {
        let byte = snap[(r_start + i) & mask];
        let sec = (byte >> 1) & 0x07;
        let val = byte & 1;
        if sec != chunk_sec {
            flush_chunk(&mut line, &mut line_len, chunk_sec, &mut sink);
            // After the first chunk, a new sector-0 entry means rev 1.
            if sec == 0 && chunks_seen > 0 {
                rev_label = 1;
            }
            let _ = write!(
                SinkFmt { sink: &mut sink },
                "[rev {} sec {}]: ",
                rev_label,
                sec
            );
            chunk_sec = sec;
            chunks_seen += 1;
        }
        if line_len >= line.len() {
            sink(&line);
            line_len = 0;
        }
        line[line_len] = if val != 0 { b'#' } else { b'.' };
        line_len += 1;
    }
    flush_chunk(&mut line, &mut line_len, chunk_sec, &mut sink);
}

/// Pre-flight verdict for the edge-buffer dump (`e`/`E` keys).
#[derive(Debug, PartialEq, Eq)]
pub enum EdgeDumpStatus {
    /// No complete rev-pair captured yet (fresh boot / just re-armed).
    Invalid,
    /// Motor off (f = 0) — the frozen half is stale.
    MotorOff,
    /// Dumpable; `window_us` is the frozen 2-rev span for the header.
    Ok { window_us: u32 },
}

/// Validity + header math for the edge dump: a half is dumpable only
/// once every sector-start slot and the end tick are populated.
pub fn edge_dump_status(
    sec_starts: &[u32; 12],
    window_end_tick: u32,
    electrical_hz: u32,
) -> EdgeDumpStatus {
    if window_end_tick == 0 || sec_starts.contains(&0) {
        EdgeDumpStatus::Invalid
    } else if electrical_hz == 0 {
        EdgeDumpStatus::MotorOff
    } else {
        EdgeDumpStatus::Ok {
            window_us: window_end_tick.wrapping_sub(sec_starts[0]).wrapping_mul(10),
        }
    }
}

/// Glyph for an edge-buffer bin count: 0 → `.`, 1 → `o`, 2 → `O`,
/// 3 → `*`, ≥4 → `#`.
pub fn edge_glyph(count: u8) -> u8 {
    match count {
        0 => b'.',
        1 => b'o',
        2 => b'O',
        3 => b'*',
        _ => b'#',
    }
}

/// Edge-buffer dump body (`e`/`E` keys): 12 sector chunks (2 revs),
/// each `[rev R sec S]: <glyphs> [edge_count]`. `snap` is the frozen
/// half's 10 µs bins (len power of two); `sec_starts` the 12 sector
/// entry ticks; `window_end_tick` the freeze tick. Header + validity
/// checks stay with the caller.
pub fn edge_dump_body<S: FnMut(&[u8])>(
    snap: &[u8],
    sec_starts: &[u32; 12],
    sec_counts: &[u32; 12],
    window_end_tick: u32,
    mut sink: S,
) {
    let mask = (snap.len() - 1) as u32;
    let window_start_tick = sec_starts[0];
    let mut line = [0u8; 256];
    for s in 0..12u32 {
        let rev = s / 6;
        let sec = s % 6;
        let start_tick = sec_starts[s as usize];
        let end_tick = if s + 1 < 12 {
            sec_starts[(s + 1) as usize]
        } else {
            window_end_tick
        };
        let start_off = (start_tick.wrapping_sub(window_start_tick) & mask) as usize;
        let end_off = (end_tick.wrapping_sub(window_start_tick) & mask) as usize;
        let _ = write!(SinkFmt { sink: &mut sink }, "[rev {} sec {}]: ", rev, sec);
        if end_off > snap.len() || end_off < start_off {
            sink(b"(range invalid)\r\n");
            continue;
        }
        let mut line_len = 0usize;
        for &bin in &snap[start_off..end_off] {
            if line_len >= line.len() {
                sink(&line);
                line_len = 0;
            }
            line[line_len] = edge_glyph(bin);
            line_len += 1;
        }
        sink(&line[..line_len]);
        let _ = write!(
            SinkFmt { sink: &mut sink },
            " [{}]\r\n",
            sec_counts[s as usize]
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn collect(f: impl FnOnce(&mut dyn FnMut(&[u8]))) -> String {
        let mut out = Vec::new();
        let mut sink = |b: &[u8]| out.extend_from_slice(b);
        f(&mut sink);
        String::from_utf8(out).unwrap()
    }

    #[test]
    fn wax_cdump_framing() {
        // 9 frames: one full 80-char line (8 frames) + one 10-char
        // partial, then "end".
        let out = collect(|sink| wax_cdump(9, |k| [k as u16, 0, 0, 0], |b| sink(b)));
        let lines: Vec<&str> = out.split("\r\n").collect();
        assert_eq!(lines[0].len(), 80);
        assert_eq!(lines[1].len(), 10);
        assert_eq!(lines[2], "end");
        // First group of frame 0: words [0,0,0,0] → bytes all zero →
        // "!!!!!" twice.
        assert_eq!(&lines[0][..10], "!!!!!!!!!!");
    }

    #[test]
    fn wax_cdump_word_packing_is_le() {
        let out = collect(|sink| wax_cdump(1, |_| [0x0102, 0x0304, 0x0506, 0x0708], |b| sink(b)));
        // LE bytes: 02 01 04 03 | 06 05 08 07, a85 groups big-endian.
        let g1 = crate::a85::encode_group([0x02, 0x01, 0x04, 0x03]);
        let g2 = crate::a85::encode_group([0x06, 0x05, 0x08, 0x07]);
        let mut expect = String::new();
        expect.push_str(core::str::from_utf8(&g1).unwrap());
        expect.push_str(core::str::from_utf8(&g2).unwrap());
        expect.push_str("\r\nend\r\n");
        assert_eq!(out, expect);
    }

    /// Ring where each sector lasts `n` samples, sectors cycling
    /// 0..=5, starting at index 0 with sector `first`.
    fn sector_ring(len: usize, n: usize, first: u8, comp_bit: u8) -> Vec<u8> {
        (0..len)
            .map(|i| {
                let sec = ((first as usize + i / n) % 6) as u8;
                (sec << 1) | comp_bit
            })
            .collect()
    }

    #[test]
    fn rev_span_finds_two_complete_revs() {
        // 64-sample ring, 3 samples/sector → rev = 18 samples; three
        // 5→0 transitions fit (at 18, 36, 54), so the last two revs
        // are [18, 54).
        let snap = sector_ring(64, 3, 0, 0);
        let (start, length) = pwm_rev_span(&snap, 0).unwrap();
        assert_eq!(length, 36, "two revs at 18 samples each");
        assert_eq!((snap[start] >> 1) & 7, 0, "span starts at sector 0");
    }

    #[test]
    fn rev_span_reports_insufficient_revs() {
        // Constant sector: no transitions at all.
        let snap = vec![0u8; 64];
        assert_eq!(pwm_rev_span(&snap, 0), Err(0));
    }

    #[test]
    fn sector_dump_chunks_and_zc_markers() {
        let snap = sector_ring(64, 3, 0, 1); // COMP=1 everywhere → '#'
        let (start, length) = pwm_rev_span(&snap, 0).unwrap();
        let out = collect(|sink| pwm_sector_dump(&snap, start, length, 2, 5, |b| sink(b)));
        // 12 chunks, labelled rev 0 then rev 1.
        assert_eq!(out.matches("[rev 0 sec").count(), 6);
        assert_eq!(out.matches("[rev 1 sec").count(), 6);
        // Float sectors 2 and 5 get the midpoint marker; '#' → '*'.
        for l in out.split("\r\n") {
            if l.starts_with("[rev") && (l.contains("sec 2]") || l.contains("sec 5]")) {
                assert!(l.contains('*'), "no ZC marker in: {l}");
            } else if l.starts_with("[rev") {
                assert!(!l.contains('*') && !l.contains('o'), "marker leaked: {l}");
            }
        }
    }

    #[test]
    fn edge_dump_glyphs_and_counts() {
        let mut snap = vec![0u8; 128];
        snap[2] = 1; // 'o' in sector 0
        snap[3] = 4; // '#'
        // 12 sectors of 10 ticks each: starts 100, 110, ... 210,
        // window end 220.
        let mut starts = [0u32; 12];
        for (i, s) in starts.iter_mut().enumerate() {
            *s = 100 + 10 * i as u32;
        }
        let mut counts = [0u32; 12];
        counts[0] = 2;
        let out = collect(|sink| edge_dump_body(&snap, &starts, &counts, 220, |b| sink(b)));
        let first = out.split("\r\n").next().unwrap();
        assert_eq!(first, "[rev 0 sec 0]: ..o#...... [2]");
        assert_eq!(out.matches("[rev").count(), 12);
    }

    #[test]
    fn edge_dump_status_gates() {
        let mut starts = [1u32; 12];
        assert_eq!(
            edge_dump_status(&starts, 500, 100),
            EdgeDumpStatus::Ok { window_us: 4990 }
        );
        assert_eq!(edge_dump_status(&starts, 500, 0), EdgeDumpStatus::MotorOff);
        assert_eq!(edge_dump_status(&starts, 0, 100), EdgeDumpStatus::Invalid);
        starts[7] = 0; // one unpopulated sector slot invalidates
        assert_eq!(edge_dump_status(&starts, 500, 100), EdgeDumpStatus::Invalid);
        // Tick-counter wrap mid-window still yields the right span.
        let starts = [u32::MAX - 9; 12];
        assert_eq!(
            edge_dump_status(&starts, 40, 100),
            EdgeDumpStatus::Ok { window_us: 500 }
        );
    }

    #[test]
    fn edge_glyph_scale() {
        assert_eq!(
            [0u8, 1, 2, 3, 4, 200].map(edge_glyph),
            [b'.', b'o', b'O', b'*', b'#', b'#']
        );
    }
}

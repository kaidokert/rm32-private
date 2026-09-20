//! Allocation-free Ascii85 snapshot records, only emitted with bridge disabled.
//! Explicit LE u16 payload, LE CRC32/ISO-HDLC trailer; no struct padding on wire.
use core::fmt::Write;

#[inline(never)]
pub fn crc32(bytes: &[u8]) -> u32 {
    let mut crc = !0u32;
    for &byte in bytes {
        crc ^= byte as u32;
        for _ in 0..8 {
            crc = (crc >> 1) ^ (0xedb88320 & 0u32.wrapping_sub(crc & 1));
        }
    }
    !crc
}

// Post-stop transport only: share encoding instead of expanding at each caller.
#[inline(never)]
pub fn record<W: Write>(out: &mut W, kind: &str, words: &[u16]) -> core::fmt::Result {
    assert!(words.len() <= 16);
    let mut bytes = [0u8; 36];
    for (i, value) in words.iter().enumerate() {
        bytes[2 * i..2 * i + 2].copy_from_slice(&value.to_le_bytes());
    }
    let size = words.len() * 2;
    let crc = crc32(&bytes[..size]);
    bytes[size..size + 4].copy_from_slice(&crc.to_le_bytes());
    write!(out, "{} ", kind)?;
    for chunk in bytes[..size + 4].chunks(4) {
        let mut padded = [0u8; 4];
        padded[..chunk.len()].copy_from_slice(chunk);
        let mut value = u32::from_be_bytes(padded);
        let mut encoded = [b'!'; 5];
        for digit in encoded.iter_mut().rev() {
            *digit += (value % 85) as u8;
            value /= 85;
        }
        // Always 33..117 ASCII, including partial final blocks. No z shorthand.
        out.write_str(core::str::from_utf8(&encoded[..chunk.len() + 1]).unwrap())?;
    }
    out.write_char('\n')
}

#[cfg(test)]
mod tests {
    #[test]
    fn known_crc() {
        assert_eq!(super::crc32(b"123456789"), 0xcbf43926);
    }
    #[test]
    fn python_compatible_vector() {
        let mut out = String::new();
        super::record(&mut out, "D85", &[0, 1, 65535]).unwrap();
        println!("{}", out.trim());
        assert_eq!(out, "D85 !!!$\"s8P+/=LS\n");
    }
}

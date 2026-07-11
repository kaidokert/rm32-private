//! Minimal Ascii85 encoder — standard `!`-based alphabet, big-endian
//! group packing, no `z` shortcut, no `<~ ~>` framing. Byte-compatible
//! with Python's `base64.a85decode` defaults, and with the rinz bench
//! dump format (`cdump: ... b85`).

/// Encode one 4-byte group into 5 Ascii85 characters.
#[inline]
pub fn encode_group(bytes: [u8; 4]) -> [u8; 5] {
    let mut v = u32::from_be_bytes(bytes);
    let mut out = [0u8; 5];
    for slot in out.iter_mut().rev() {
        *slot = b'!' + (v % 85) as u8;
        v /= 85;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matches_python_a85decode() {
        // b"Man " encodes to "9jqo^" in standard Ascii85 (the classic
        // Wikipedia vector).
        assert_eq!(&encode_group(*b"Man "), b"9jqo^");
        assert_eq!(&encode_group([0, 0, 0, 0]), b"!!!!!");
        assert_eq!(&encode_group([0xFF; 4]), b"s8W-!");
    }
}

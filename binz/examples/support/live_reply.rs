//! One bounded reply, no formatting/division and no UART waits. New requests
//! while a reply is pending are refused rather than splicing two replies.
pub struct Reply {
    bytes: [u8; 16],
    next: usize,
}
impl Reply {
    pub const fn new() -> Self {
        Self {
            bytes: [0; 16],
            next: 16,
        }
    }
    pub fn queue(&mut self, duty: u32, interval: u32) -> bool {
        if self.next != 16 {
            return false;
        }
        self.bytes = *b"D=0000 I=0000 \r\n";
        const HEX: &[u8; 16] = b"0123456789ABCDEF";
        for (offset, value) in [(2, duty), (9, interval)] {
            for i in 0..4 {
                self.bytes[offset + i] = HEX[((value >> (12 - i * 4)) & 15) as usize];
            }
        }
        self.next = 0;
        true
    }
    pub fn byte(&self) -> Option<u8> {
        self.bytes.get(self.next).copied()
    }
    pub fn sent(&mut self) {
        if self.next < 16 {
            self.next += 1;
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn bounded_no_partial_overwrite() {
        let mut r = Reply::new();
        assert_eq!(r.byte(), None);
        assert!(r.queue(300, 1000));
        assert!(!r.queue(70, 123));
        let mut out = Vec::new();
        while let Some(b) = r.byte() {
            out.push(b);
            r.sent();
        }
        assert_eq!(out, b"D=012C I=03E8 \r\n");
        r.sent();
        assert!(r.queue(70, 0));
    }
}

//! Binary telemetry frame for the 2 Mbaud VCOM stream.
//!
//! 22-byte frame, little-endian, XOR checksum:
//!   [0]  0x5B  [1] 0xA9      sync
//!   [2]  seq (u8, wraps)
//!   [3]  state: low nibble = rung index, bit6 = coasting, bit7 = killed
//!   [4:6]  drive phase (u16, top 16 bits of the accumulator)
//!   [6:8]  vph1 mV   [8:10] vph2 mV   [10:12] vph3 mV
//!   [12:14] is mV    [14:16] vm mV
//!   [16:18] freq centi-Hz (u16)
//!   [18:20] amp (CCR counts)
//!   [20] xor of bytes 2..20   [21] 0x0A (also keeps text logs readable)

pub const FRAME_LEN: usize = 22;
pub const SYNC0: u8 = 0x5B;
pub const SYNC1: u8 = 0xA9;

#[allow(clippy::too_many_arguments)]
pub fn build_frame(
    seq: u8,
    state: u8,
    phase_hi: u16,
    vph1: u16,
    vph2: u16,
    vph3: u16,
    is: u16,
    vm: u16,
    freq_chz: u16,
    amp: u16,
) -> [u8; FRAME_LEN] {
    let mut f = [0u8; FRAME_LEN];
    f[0] = SYNC0;
    f[1] = SYNC1;
    f[2] = seq;
    f[3] = state;
    f[4..6].copy_from_slice(&phase_hi.to_le_bytes());
    f[6..8].copy_from_slice(&vph1.to_le_bytes());
    f[8..10].copy_from_slice(&vph2.to_le_bytes());
    f[10..12].copy_from_slice(&vph3.to_le_bytes());
    f[12..14].copy_from_slice(&is.to_le_bytes());
    f[14..16].copy_from_slice(&vm.to_le_bytes());
    f[16..18].copy_from_slice(&freq_chz.to_le_bytes());
    f[18..20].copy_from_slice(&amp.to_le_bytes());
    let mut x = 0u8;
    for b in &f[2..20] {
        x ^= *b;
    }
    f[20] = x;
    f[21] = 0x0A;
    f
}

/// Tiny software TX ring drained a few bytes per control tick so frame
/// writes never block the 10 kHz loop (UART FIFO is only 8 deep).
pub struct TxRing {
    buf: [u8; 256],
    head: usize,
    tail: usize,
}

impl TxRing {
    pub const fn new() -> Self {
        Self {
            buf: [0; 256],
            head: 0,
            tail: 0,
        }
    }
    pub fn push(&mut self, bytes: &[u8]) -> bool {
        let free = 255 - (self.head.wrapping_sub(self.tail) & 255);
        if bytes.len() > free {
            return false; // drop whole frame; seq gap shows the loss
        }
        for &b in bytes {
            self.buf[self.head & 255] = b;
            self.head = self.head.wrapping_add(1);
        }
        true
    }
    pub fn is_empty(&self) -> bool {
        self.head == self.tail
    }
    /// Pop up to `n` bytes, feeding them through `tx` (returns false when
    /// the UART can't take more right now).
    pub fn drain<F: FnMut(u8) -> bool>(&mut self, n: usize, mut tx: F) {
        for _ in 0..n {
            if self.head == self.tail {
                return;
            }
            let b = self.buf[self.tail & 255];
            if !tx(b) {
                return;
            }
            self.tail = self.tail.wrapping_add(1);
        }
    }
}

impl Default for TxRing {
    fn default() -> Self {
        Self::new()
    }
}

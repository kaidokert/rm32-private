//! Driver-disabled fixed-frame DMA roundtrip benchmark. Always restores UART.
use super::*;
use core::sync::atomic::{Ordering, compiler_fence};
const LEN: usize = 1024;
static mut BUFFER: [u8; LEN] = [0; LEN];
pub fn run<W: Write>(out: &mut W, baud: u32) {
    gates_off();
    set_pin(3, 1, false);
    let _ = writeln!(out, "DMA ready baud={} n=32 bytes=1024 switch_ms=200", baud);
    let u = unsafe { &*stm32::USART3::ptr() };
    while u.isr().read().bits() & (1 << 6) == 0 {}
    let wait = clock_us();
    while clock_us().wrapping_sub(wait) < 200_000 {}
    let (cr1, cr3, brr) = (
        u.cr1().read().bits(),
        u.cr3().read().bits(),
        u.brr().read().bits(),
    );
    let dma = unsafe { &*stm32::DMA1::ptr() };
    let ch = dma.ch1();
    let mux = unsafe { &*stm32::DMAMUX::ptr() };
    let ptr = core::ptr::addr_of_mut!(BUFFER).cast::<u8>();
    let mut completed = 0;
    let mut bad_crc = 0;
    let mut errors = 0;
    let mut timedout = 0;
    unsafe {
        (*stm32::RCC::ptr())
            .ahbenr()
            .modify(|r, w| w.bits(r.bits() | 1));
        u.cr1().write(|w| w.bits(cr1 & !1));
        let div = (64_000_000 + baud / 2) / baud;
        let over8 = baud > 4_000_000;
        u.brr().write(|w| {
            w.bits(if over8 {
                ((div & !7) << 1) | (div & 7)
            } else {
                div
            })
        });
        u.icr().write(|w| w.bits(0x0012_1b5f));
        u.rqr().write(|w| w.bits(1 << 3));
        u.cr3().write(|w| w.bits(cr3 | (1 << 6) | (1 << 7)));
        u.cr1()
            .write(|w| w.bits((cr1 & !(1 << 15)) | if over8 { 1 << 15 } else { 0 }));
    }
    let started = clock_us();
    'frames: for _ in 0..32 {
        for tx in [false, true] {
            unsafe {
                ch.cr().write(|w| w.bits(0));
                dma.ifcr().write(|w| w.bits(15));
                mux.ccr(0).write(|w| w.bits(if tx { 55 } else { 54 }));
                ch.par().write(|w| {
                    w.bits(if tx {
                        u.tdr().as_ptr() as u32
                    } else {
                        u.rdr().as_ptr() as u32
                    })
                });
                ch.mar().write(|w| w.bits(ptr as u32));
                ch.ndtr().write(|w| w.bits(LEN as u32));
                if tx {
                    u.icr().write(|w| w.bits(1 << 6));
                }
                compiler_fence(Ordering::SeqCst);
                ch.cr()
                    .write(|w| w.bits(1 | (1 << 7) | if tx { 1 << 4 } else { 0 }));
            }
            let start = clock_us();
            while dma.isr().read().bits() & 2 == 0 {
                if dma.isr().read().bits() & 8 != 0 {
                    errors += 1;
                    break 'frames;
                }
                if clock_us().wrapping_sub(start) > 2_000_000 {
                    timedout += 1;
                    break 'frames;
                }
            }
            unsafe {
                ch.cr().write(|w| w.bits(0));
            }
            compiler_fence(Ordering::SeqCst);
            errors += u.isr().read().bits() & 15;
            unsafe {
                u.icr().write(|w| w.bits(15));
            }
            if !tx {
                let bytes = unsafe { core::slice::from_raw_parts_mut(ptr, LEN) };
                let crc = u32::from_le_bytes(bytes[1020..].try_into().unwrap());
                if crc != snapshot::crc32(&bytes[..1020]) {
                    bad_crc += 1;
                    bytes[..4].copy_from_slice(b"ERR!");
                }
                for b in &mut bytes[8..1020] {
                    *b ^= 0xa5;
                }
                let reply = snapshot::crc32(&bytes[..1020]);
                bytes[1020..].copy_from_slice(&reply.to_le_bytes());
            } else {
                let start = clock_us();
                while u.isr().read().bits() & (1 << 6) == 0 {
                    if clock_us().wrapping_sub(start) > 2_000_000 {
                        timedout += 1;
                        break 'frames;
                    }
                }
            }
        }
        completed += 1;
    }
    let elapsed = clock_us().wrapping_sub(started);
    unsafe {
        ch.cr().write(|w| w.bits(0));
        dma.ifcr().write(|w| w.bits(15));
        mux.ccr(0).write(|w| w.bits(0));
        u.cr1().write(|w| w.bits(cr1 & !1));
        u.cr3().write(|w| w.bits(cr3));
        u.brr().write(|w| w.bits(brr));
        u.icr().write(|w| w.bits(0x0012_1b5f));
        u.rqr().write(|w| w.bits(1 << 3));
        u.cr1().write(|w| w.bits(cr1));
    }
    gates_off();
    set_pin(3, 1, false);
    let pause = clock_us();
    while clock_us().wrapping_sub(pause) < 200_000 {}
    let _ = writeln!(
        out,
        "DMA done frames={} rx_crc_bad={} uart_error_bits_sum={} timeout={} elapsed_us={} restored=115200",
        completed, bad_crc, errors, timedout, elapsed
    );
}

//! binz bench: measured cost of one BEMF persistence-filter read, rm32 vs
//! AM32, on this chip's flash (64 MHz, 2 wait states, prefetch on/off).
//!
//! AM32's loop is reproduced instruction for instruction from
//! `AM32_DRV8304H_G071_2.20.elf` (`interruptRoutine` 0x8000e9c..0x8000eac
//! calling `getCompOutputLevel` 0x80053b4, including its flash literal and
//! the loop's mod-8 flash alignment); rm32's from its own `ADC_COMP` filter
//! loop with a variable NOP pad. Each is timed for 64 and 32 full-length
//! reads (comparator level held opposite to `rising`, so no early exit) on
//! TIM6's CPU-cycle counter with interrupts off; per read = (t64 - t32) / 32.
//! Run once at boot; prints one line per prefetch state.

core::arch::global_asm!(
    r#"
    .syntax unified
    .thumb

    .section .text.cal_am32_get,"ax",%progbits
    .balign 8
    nop
    nop
    .thumb_func
    .global cal_am32_get
cal_am32_get:
    ldr r3, [pc, #8]
    ldr r3, [r3, #0]
    ldr r0, [r3, #0]
    lsls r0, r0, #1
    lsrs r0, r0, #31
    bx lr
    .word CAL_AM32_ACTIVE

    .section .text.cal_am32_loop,"ax",%progbits
    .balign 8
    .thumb_func
    .global cal_am32_loop
cal_am32_loop:
    push {{r3, r4, r5, r6, r7, lr}}
    mov r5, r0
    movs r4, #0
    movs r6, #80
    movs r7, #100
    b 2f
1:  ldrb r3, [r5, r7]
    adds r4, #1
    cmp r3, r4
    ble 3f
2:  bl cal_am32_get
    ldrb r3, [r5, r6]
    cmp r0, r3
    bne 1b
3:  mov r0, r4
    pop {{r3, r4, r5, r6, r7, pc}}

    .macro RM32LOOP name, k
    .section .text.\name,"ax",%progbits
    .balign 8
    .thumb_func
    .global \name
\name:
    push {{r4, r5, r6, lr}}
    mov r5, r2
    movs r2, r0
    movs r4, #1
    lsls r4, r4, #30
    movs r3, #0
    nop
1:  mov r6, r3
    cmp r3, r2
    bcs 2f
    .rept \k
    nop
    .endr
    ldr r0, [r1, #0]
    ands r0, r4
    subs r3, r0, #1
    sbcs r0, r3
    eors r0, r5
    adds r3, r6, #1
    cmp r0, #0
    bne 1b
2:  mov r0, r6
    pop {{r4, r5, r6, pc}}
    .endm

    RM32LOOP cal_rm32_k0, 0
    RM32LOOP cal_rm32_k8, 8
    RM32LOOP cal_rm32_k12, 12
    RM32LOOP cal_rm32_k16, 16
    RM32LOOP cal_rm32_k20, 20
    RM32LOOP cal_rm32_k24, 24
    RM32LOOP cal_rm32_k28, 28
    "#
);

extern "C" {
    fn cal_am32_loop(globals: *const u8) -> u32;
    fn cal_rm32_k0(n: u32, csr: u32, rising: u32) -> u32;
    fn cal_rm32_k8(n: u32, csr: u32, rising: u32) -> u32;
    fn cal_rm32_k12(n: u32, csr: u32, rising: u32) -> u32;
    fn cal_rm32_k16(n: u32, csr: u32, rising: u32) -> u32;
    fn cal_rm32_k20(n: u32, csr: u32, rising: u32) -> u32;
    fn cal_rm32_k24(n: u32, csr: u32, rising: u32) -> u32;
    fn cal_rm32_k28(n: u32, csr: u32, rising: u32) -> u32;
}

/// AM32's `active_COMP` (a pointer to COMP2's CSR), read through the callee's
/// literal exactly as AM32 does.
#[no_mangle]
static mut CAL_AM32_ACTIVE: u32 = 0;
/// AM32's globals block as `interruptRoutine` addresses it: `rising` at +80,
/// `filter_level` at +100 (both `ldrb [r5, #imm]`).
static mut CAL_AM32_GLOBALS: [u8; 128] = [0; 128];

fn cnt() -> u32 {
    unsafe { (*stm32g0xx_hal::stm32::TIM6::ptr()).cnt().read().bits() }
}

fn elapsed(t0: u32, t1: u32) -> u32 {
    if t1 >= t0 { t1 - t0 } else { t1 + 3200 - t0 }
}

/// Cycles x10 per read, and the iteration counts (sanity: must equal n).
fn per_read_x10(f: &dyn Fn(u32) -> u32) -> (u32, u32) {
    let run = |n: u32| {
        let t0 = cnt();
        let it = f(n);
        (elapsed(t0, cnt()), it)
    };
    let _ = run(32); // warm the prefetch/cache path once
    let (t64, i64) = run(64);
    let (t32, _) = run(32);
    (t64.saturating_sub(t32) * 10 / 32, i64)
}

pub fn run() {
    let csr = unsafe { &(*stm32g0xx_hal::stm32::COMP::ptr()).comp2_csr() as *const _ as u32 };
    let level = unsafe { core::ptr::read_volatile(csr as *const u32) } >> 30 & 1;
    let rising = level ^ 1; // never matches: every run reads n times
    let flash = unsafe { &*stm32g0xx_hal::stm32::FLASH::ptr() };
    let acr0 = flash.acr().read().bits();
    for pf in [true, false] {
        let acr = if pf {
            acr0 | (1 << 8)
        } else {
            acr0 & !(1 << 8)
        };
        let (am, rm) = cortex_m::interrupt::free(|_| unsafe {
            flash.acr().write(|w| w.bits(acr));
            CAL_AM32_ACTIVE = csr;
            let g = core::ptr::addr_of_mut!(CAL_AM32_GLOBALS) as *mut u8;
            *g.add(80) = rising as u8;
            let am = per_read_x10(&|n| {
                *g.add(100) = n as u8;
                cal_am32_loop(g)
            });
            let fns: [unsafe extern "C" fn(u32, u32, u32) -> u32; 7] = [
                cal_rm32_k0,
                cal_rm32_k8,
                cal_rm32_k12,
                cal_rm32_k16,
                cal_rm32_k20,
                cal_rm32_k24,
                cal_rm32_k28,
            ];
            let rm = fns.map(|f| per_read_x10(&|n| f(n, csr, rising)).0);
            (am, rm)
        });
        crate::dprintln!(
            "[bench] filter-read cyc x10 pf={} am32={} (it={}) rm32 k0={} k8={} k12={} k16={} k20={} k24={} k28={}",
            pf as u8,
            am.0,
            am.1,
            rm[0],
            rm[1],
            rm[2],
            rm[3],
            rm[4],
            rm[5],
            rm[6]
        );
    }
    flash.acr().write(|w| unsafe { w.bits(acr0) });
}

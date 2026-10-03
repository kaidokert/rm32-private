//! Fault-triggered three-scan snapshot. No change to the sag decision.
//! DMA alone writes these rows; foreground only reads after powered stop.
use core::fmt::Write;

// acquired, service, bus, vref, IA, IB, IC, duty, last accepted event,
// guard sector, average half-us, measured half-us, TIM2 count, core step,
// commits. Three rows: the latest consecutive sub-95% scans.
static mut ROWS: [[u32; 15]; 3] = [[0; 15]; 3];
static mut ROWS_N: u32 = 0;
static mut EVENT_RING: [u32; 8] = [0; 8];
static mut EVENT_HEAD: u32 = 0;
static mut FIRST_EVENTS: [u32; 8] = [0; 8];
static mut THIRD_EVENTS: [u32; 8] = [0; 8];
// DMA alone writes this bounded ring. It is frozen at the third low scan,
// preserving five preceding scans plus the three that caused the stop.
// Columns: scan origin, physical IA, IB, IC, bus, VREF ADC codes.
// `adc_stream::poll` has ALREADY remapped ADC0/1/4 through
// `dma_snapshot::logical` before this module receives `raw`.
static mut SCAN_RING: [[u32; 6]; 8] = [[0; 6]; 8];
static mut SCAN_HEAD: u32 = 0;
static mut SCAN_COUNT: u32 = 0;
static mut SCAN_FROZEN: bool = false;
// Captured only at the terminal third low scan. The timestamp and TIM1 CNT
// are read back-to-back before shutdown; all earlier scan phases can then be
// reconstructed offline without touching the normal DMA scan path.
static mut PWM_STOP: [u32; 7] = [0; 7];

#[inline(always)]
pub fn pwm_stop(row: [u32; 7]) {
    unsafe {
        core::ptr::addr_of_mut!(PWM_STOP).write(row);
    }
}

#[inline(always)]
pub fn scan(acquired: u32, raw: [u16; 5], streak: u8) {
    unsafe {
        if core::ptr::addr_of!(SCAN_FROZEN).read() {
            return;
        }
        let head = core::ptr::addr_of!(SCAN_HEAD).read();
        core::ptr::addr_of_mut!(SCAN_RING)
            .cast::<[u32; 6]>()
            .add((head as usize) & 7)
            .write([
                acquired,
                raw[0] as u32,
                raw[1] as u32,
                raw[2] as u32,
                raw[3] as u32,
                raw[4] as u32,
            ]);
        core::ptr::addr_of_mut!(SCAN_HEAD).write(head.wrapping_add(1));
        let count = core::ptr::addr_of!(SCAN_COUNT).read();
        if count < 8 {
            core::ptr::addr_of_mut!(SCAN_COUNT).write(count + 1);
        }
        if streak == 3 {
            core::ptr::addr_of_mut!(SCAN_FROZEN).write(true);
        }
    }
}

/// The powered acceptance path already owns `at`; one indexed store and
/// one ordinal store add no clock read, division, or guard decision.
#[inline(always)]
pub fn accepted(at: u32) {
    unsafe {
        let head = core::ptr::addr_of!(EVENT_HEAD).read();
        core::ptr::addr_of_mut!(EVENT_RING)
            .cast::<u32>()
            .add((head as usize) & 7)
            .write(at);
        core::ptr::addr_of_mut!(EVENT_HEAD).write(head.wrapping_add(1));
    }
}

#[inline(always)]
unsafe fn recent() -> [u32; 8] {
    let head = unsafe { core::ptr::addr_of!(EVENT_HEAD).read() };
    let mut out = [0; 8];
    for (i, value) in out.iter_mut().enumerate() {
        *value = unsafe {
            core::ptr::addr_of!(EVENT_RING)
                .cast::<u32>()
                .add(((head as usize) + i) & 7)
                .read()
        };
    }
    out
}

pub fn reset() {
    cortex_m::interrupt::free(|_| unsafe {
        core::ptr::addr_of_mut!(ROWS).write([[0; 15]; 3]);
        core::ptr::addr_of_mut!(ROWS_N).write(0);
        core::ptr::addr_of_mut!(EVENT_RING).write([0; 8]);
        core::ptr::addr_of_mut!(EVENT_HEAD).write(0);
        core::ptr::addr_of_mut!(FIRST_EVENTS).write([0; 8]);
        core::ptr::addr_of_mut!(THIRD_EVENTS).write([0; 8]);
        core::ptr::addr_of_mut!(SCAN_RING).write([[0; 6]; 8]);
        core::ptr::addr_of_mut!(SCAN_HEAD).write(0);
        core::ptr::addr_of_mut!(SCAN_COUNT).write(0);
        core::ptr::addr_of_mut!(SCAN_FROZEN).write(false);
        core::ptr::addr_of_mut!(PWM_STOP).write([0; 7]);
    });
}

#[inline(never)]
pub fn record(
    streak: u8,
    acquired: u32,
    service: u32,
    raw: [u16; 5],
    event: [u32; 5],
    controller: [u32; 5],
    commits: u32,
) {
    if !(1..=3).contains(&streak) {
        return;
    }
    unsafe {
        if streak == 1 {
            core::ptr::addr_of_mut!(ROWS).write([[0; 15]; 3]);
            core::ptr::addr_of_mut!(FIRST_EVENTS).write(recent());
            core::ptr::addr_of_mut!(THIRD_EVENTS).write([0; 8]);
        } else if streak == 3 {
            core::ptr::addr_of_mut!(THIRD_EVENTS).write(recent());
        }
        let row = [
            acquired,
            service,
            raw[3] as u32,
            raw[4] as u32,
            raw[0] as u32,
            raw[1] as u32,
            raw[2] as u32,
            controller[4],
            event[1],
            event[2],
            controller[0],
            controller[1],
            controller[2],
            controller[3],
            commits,
        ];
        core::ptr::addr_of_mut!(ROWS)
            .cast::<[u32; 15]>()
            .add(streak as usize - 1)
            .write(row);
        core::ptr::addr_of_mut!(ROWS_N).write(streak as u32);
    }
}

pub fn dump<W: Write>(out: &mut W, reason: u32) {
    let n = unsafe { core::ptr::addr_of!(ROWS_N).read() }.min(3);
    let _ = writeln!(
        out,
        "SAGCAUSE n={} stop_reason={} fields=acquired_us,service_us,bus,vref,ia,ib,ic,duty,last_event_us,sector,average_half_us,this_zc_half_us,tim2_cnt,core_step,commits fault_triggered={} no_control_authority=1",
        n,
        reason,
        (n == 3 && reason == 26) as u8
    );
    for i in 0..n as usize {
        let r = unsafe { core::ptr::addr_of!(ROWS).cast::<[u32; 15]>().add(i).read() };
        let _ = writeln!(
            out,
            "SAGROW streak={} acquired_us={} service_us={} bus={} vref={} ia={} ib={} ic={} duty={} last_event_us={} sector={} average_half_us={} this_zc_half_us={} tim2_cnt={} core_step={} commits={}",
            i + 1,
            r[0],
            r[1],
            r[2],
            r[3],
            r[4],
            r[5],
            r[6],
            r[7],
            r[8],
            r[9],
            r[10],
            r[11],
            r[12],
            r[13],
            r[14]
        );
    }
    if n != 0 {
        let first = unsafe { core::ptr::addr_of!(FIRST_EVENTS).read() };
        let _ = writeln!(
            out,
            "SAGEVENTS at=first_low stamps_us={},{},{},{},{},{},{},{} accepted_guard_timestamps=1 chronological=1",
            first[0], first[1], first[2], first[3], first[4], first[5], first[6], first[7]
        );
    }
    if n == 3 {
        let p = unsafe { core::ptr::addr_of!(PWM_STOP).read() };
        let _ = writeln!(
            out,
            "SAGPWM seen={} stamp_us={} tim1_cnt={} tim1_arr={} ccr1={} ccr2={} ccr3={} cr1={} terminal_dma_service=1 sequential_adc_apertures=1",
            (p[2] != 0) as u8,
            p[0],
            p[1],
            p[2],
            p[3],
            p[4],
            p[5],
            p[6]
        );
        let third = unsafe { core::ptr::addr_of!(THIRD_EVENTS).read() };
        let _ = writeln!(
            out,
            "SAGEVENTS at=third_low stamps_us={},{},{},{},{},{},{},{} accepted_guard_timestamps=1 chronological=1",
            third[0], third[1], third[2], third[3], third[4], third[5], third[6], third[7]
        );
        let count = unsafe { core::ptr::addr_of!(SCAN_COUNT).read() }.min(8);
        let head = unsafe { core::ptr::addr_of!(SCAN_HEAD).read() };
        let _ = writeln!(
            out,
            "SAGSCANS n={} frozen={} fields=acquired_us,ia,ib,ic,bus,vref chronological=1",
            count,
            unsafe { core::ptr::addr_of!(SCAN_FROZEN).read() } as u8
        );
        for i in 0..count {
            let index = (head.wrapping_sub(count).wrapping_add(i) as usize) & 7;
            let row = unsafe {
                core::ptr::addr_of!(SCAN_RING)
                    .cast::<[u32; 6]>()
                    .add(index)
                    .read()
            };
            let _ = writeln!(
                out,
                "SAGSCAN ordinal={} acquired_us={} ia={} ib={} ic={} bus={} vref={}",
                i, row[0], row[1], row[2], row[3], row[4], row[5]
            );
        }
    }
}

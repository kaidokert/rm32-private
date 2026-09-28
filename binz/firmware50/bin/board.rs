//! The hardware both firmware images run on: `Board` (clocks, pins, USART3,
//! timers, ADC and DMA, TIM1, the watchdog) and its `firmware50::run::Hal`
//! and `Sink` implementations; bring-up and the boot banner; the DMA, COM and
//! guard interrupt shims; and the panic handler. Included by `shell-pwm`
//! (production) and `edge-capture` (the replay capture), which differ only in
//! their COMP shim and what `main` does.

#![allow(clippy::wildcard_imports)]

use portable_atomic::Ordering;
use stm32g0xx_hal as hal;

use hal::prelude::*;
use hal::rcc::Config;
use hal::serial::BasicConfig;
use hal::stm32;
use hal::stm32::interrupt;
use hal::timer::Timer;
use hal::watchdog::IndependedWatchdog;

use firmware50::bemf::ZeroCross;
use firmware50::bridge::safe_off;
use firmware50::commutation::{SixSlot, Step};
use firmware50::duty::{ENVELOPE_MAX, ENVELOPE_MIN, ENVELOPE_STEP, STARTUP_TICKS};
use firmware50::hw;
use firmware50::protection::{RAW_LIMIT, RawScan};
use firmware50::report::{GuardRecord, Roots, Sink};
use firmware50::roots::{self, DRV_GATE_US, Drv8304};
use firmware50::run::hal::{DrivenAccept, Drives, Stopped};
use firmware50::run::{Gates, Hal, Inject, Preflight};
use firmware50::shared::{SHARED as S, TxRing};
use firmware50::sixstep::Plan;
use firmware50::startup::{CONTROL_HZ, SCRIPT_TICKS};

/// IWDG reload for 50 ms at PR = 0, where the counter runs at LSI/4 = 8 kHz
/// (RM0444 IWDG prescaler: /4 at PR = 0). LSI's datasheet spread puts the
/// actual timeout at roughly 47-54 ms.
const IWDG_RELOAD: u16 = 400;

/// Longest gap between DMA scans before the ADC is declared stale, µs:
/// about twenty missed 101 µs scans, so a halted ADC or DMA stops the bridge
/// within two milliseconds rather than leaving the protections blind.
const ADC_STALE_US: u32 = 2_000;

/// Dead-time generator setting, in TIM1 clock ticks.
const DTG: u8 = 26;

// ---------------------------------------------------------------------------
// The four motor ISR roots (their logic is `firmware50::roots`)
// ---------------------------------------------------------------------------

/// DMA1 channel 1 transfer complete: one full scan has landed; acknowledge
/// every flag (`hw::adc::dma_isr`) and publish it under the seqlock.
#[interrupt]
fn DMA1_CHANNEL1() {
    // Q60-1: count a dropped conversion before publishing. A drop rotates the
    // circular buffer permanently, so this is the only place the cause is
    // visible; the *consequence* (a wrong value in the vref slot) is checked in
    // the foreground, where `filt_vref` exists.
    if hw::adc::adc_overran() {
        S.guard().adc_ovr.fetch_add(1, Ordering::Relaxed);
    }
    S.scan().publish(&hw::adc::dma_isr());
}

// The TIM16 shim lives in each binary, as `ADC_COMP`'s already did: both roots
// are now generic over the chain recorder (`firmware50::chain`), and the
// diagnostic image is the only one that installs it (E154).

/// The guard tick: `firmware50::roots::guard_root`.
#[interrupt]
fn TIM6_DAC_LPTIM1() {
    // SAFETY: the PAC's vector `TIM6_DAC_LPTIM1 = 17` ("17 - TIM6 + LPTIM1 and DAC global interrupt"), at `GUARD_IRQ_PRIORITY`.
    unsafe { roots::guard_root() }
}

// ---------------------------------------------------------------------------
// Panic handler: safe the bridge, record where, then stop.
// ---------------------------------------------------------------------------

#[panic_handler]
fn panic(info: &core::panic::PanicInfo) -> ! {
    cortex_m::interrupt::disable();
    safe_off(&mut Drv8304);
    let line = info.location().map_or(u32::MAX, core::panic::Location::line);
    S.panic_line().store(line, Ordering::Relaxed);
    // Deliberately `nop`, never `wfi`: WFI breaks debug access on this board.
    loop {
        cortex_m::asm::nop();
    }
}

// ---------------------------------------------------------------------------
// Board: the hardware the run is given
// ---------------------------------------------------------------------------

/// The UART transmit ring. Private to this binary and written only through
/// `Board`'s `Sink`, so nothing in the library -- in particular the `Locked`
/// state, which holds no `Sink` -- can queue a byte (E073, goal item 5).
static TX: TxRing = TxRing::new();

type Uart = hal::serial::Serial<stm32::USART3, BasicConfig>;

pub struct Board {
    uart: Uart,
    /// TIM17 as a 1 MHz free-running counter (PSC=63 / ARR=65535).
    clock: Timer<stm32::TIM17>,
    /// TIM6: its update event triggers each ADC scan in hardware (E055), and
    /// its update interrupt is the guard root's tick (E076).
    pace: Timer<stm32::TIM6>,
    /// Last DMA scan sequence consumed, and when a new one was last seen.
    adc_seq_seen: u32,
    adc_last_us: u32,
    /// Last driven / closed-loop acceptance the foreground consumed.
    drv_seq_seen: u32,
    det_seq_seen: u32,
    // The analog pins are held, not read: keeping the objects alive keeps
    // the pins in analog mode and unavailable for any other use.
    _analog: AnalogPins,
    led: hal::gpio::gpiob::PB5<hal::gpio::Output<hal::gpio::PushPull>>,
    /// Software-extended µs clock over the 16-bit TIM17.
    clock_us: u32,
    last_cnt: u32,
    rx_overruns: u32,
    /// Hardware watchdog, fed only by `tick_clock`: every path that keeps
    /// time keeps the board alive, and any path that stops doing so resets it.
    dog: IndependedWatchdog,
}

/// PA0/1/4 (shunts), PA6 (bus), PA2/PA3 (phase C / star) and PB3/PB7
/// (phases A/B, comparator-only) in analog mode.
struct AnalogPins {
    _pa0: hal::gpio::gpioa::PA0<hal::gpio::Analog>,
    _pa1: hal::gpio::gpioa::PA1<hal::gpio::Analog>,
    _pa4: hal::gpio::gpioa::PA4<hal::gpio::Analog>,
    _pa6: hal::gpio::gpioa::PA6<hal::gpio::Analog>,
    _pa2: hal::gpio::gpioa::PA2<hal::gpio::Analog>,
    _pa3: hal::gpio::gpioa::PA3<hal::gpio::Analog>,
    _pb3: hal::gpio::gpiob::PB3<hal::gpio::Analog>,
    _pb7: hal::gpio::gpiob::PB7<hal::gpio::Analog>,
}

impl Board {
    /// Advance the extended clock, feed the watchdog, and return µs.
    pub fn tick_clock(&mut self) -> u32 {
        self.dog.feed();
        let cnt = self.clock.get_current() & 0xFFFF;
        let delta = cnt.wrapping_sub(self.last_cnt) & 0xFFFF;
        self.last_cnt = cnt;
        self.clock_us = self.clock_us.wrapping_add(delta);
        self.clock_us
    }

    /// Push at most one queued byte. Inlined: it runs once per loop pass,
    /// and the run loop's speed sets the level-revisit cadence (E116).
    #[inline(always)]
    pub fn tx_drain(&mut self) {
        let Some(b) = TX.peek() else {
            return;
        };
        if self.uart.write(b).is_ok() {
            let _ = TX.pop();
        }
    }

    /// Drain the ring to completion, outside a run, advancing the clock
    /// (which feeds the dog; E089).
    pub fn tx_flush(&mut self) {
        let mut guard = 0u32;
        while guard < 20_000_000 {
            self.tick_clock();
            if TX.is_empty() {
                return;
            }
            self.tx_drain();
            guard += 1;
        }
    }
}

impl Sink for Board {
    #[inline]
    fn put(&mut self, b: u8) {
        TX.push(b);
    }

    fn flush(&mut self) {
        self.tx_flush();
    }
}

/// A guard stop dominates any foreground write resumed after that stop.
#[inline(always)]
fn powered_write(write: impl FnOnce()) {
    cortex_m::interrupt::free(|_| {
        if firmware50::oneshot::arm_allowed(roots::guard_latched(), S.guard().active.load(Ordering::Relaxed)) {
            write();
        }
    });
}

impl Hal for Board {
    #[inline(always)]
    fn now(&mut self) -> u32 {
        self.tick_clock()
    }

    #[inline(always)]
    /// The fine diagnostic clock, 125 ns a tick.
    ///
    /// **This override is the difference between a timeline and a column of
    /// zeros** (E266): `Hal::fine` has a default body returning 0, and without
    /// this the sag ring recorded no time at all while every unit-versioning
    /// check downstream passed happily on the zeros.
    ///
    /// Returns 0 unless some image called `hw::fine::init()`, because TIM2 is
    /// otherwise unclocked -- production never touches it, which is what keeps
    /// its four roots identical.
    fn fine(&self) -> u16 {
        firmware50::hw::fine::raw() as u16
    }

    fn raw(&self) -> u16 {
        hw::clock::raw()
    }

    /// A raw TIM17 count from within one wrap of now, on the extended clock:
    /// exact to 1 µs, so main-loop latency never enters an interval.
    fn stamp_from_raw(&mut self, raw: u16) -> u32 {
        let now = self.tick_clock();
        let back = u32::from((self.last_cnt as u16).wrapping_sub(raw));
        now.wrapping_sub(back)
    }

    #[inline(always)]
    fn drain(&mut self) {
        self.tx_drain();
    }

    fn flush_link(&mut self) {
        self.tx_flush();
    }

    /// Non-blocking receive; the HAL reports and clears `ORE`.
    fn rx(&mut self) -> Option<u8> {
        match self.uart.read() {
            Ok(b) => Some(b),
            Err(hal::nb::Error::Other(_)) => {
                self.rx_overruns = self.rx_overruns.wrapping_add(1);
                None
            }
            Err(hal::nb::Error::WouldBlock) => None,
        }
    }

    fn led(&mut self, on: bool) {
        if on {
            self.led.set_high().ok();
        } else {
            self.led.set_low().ok();
        }
    }

    #[inline(always)]
    fn nfault_high(&self) -> bool {
        roots::nfault_high()
    }

    fn enable(&mut self, on: bool) {
        roots::enable_set(on);
    }

    fn preflight(&self) -> Preflight {
        let (c1, c2, c3) = hw::pwm::compares();
        Preflight {
            moe: hw::pwm::moe_is_set(),
            ccr: [c1, c2, c3],
            gates_low: roots::gates_all_low(),
            enable: roots::enable_is_high(),
            nfault_high: roots::nfault_high(),
        }
    }

    /// A scan the foreground has not consumed yet (the DMA root publishes
    /// each completed scan through `S.scan()`).
    #[inline]
    fn adc_due(&mut self) -> bool {
        let s = S.scan().sequence(Ordering::SeqCst);
        if s & 1 == 0 && s != self.adc_seq_seen {
            self.adc_last_us = self.clock_us;
            true
        } else {
            false
        }
    }

    #[inline]
    fn adc_stale(&self, now: u32) -> bool {
        now.wrapping_sub(self.adc_last_us) > ADC_STALE_US
    }

    fn scan(&mut self) -> Option<RawScan> {
        let (seq, scan) = S.scan().snapshot()?;
        self.adc_seq_seen = seq;
        Some(scan)
    }

    fn resync_adc(&mut self) {
        hw::adc::resync();
    }

    fn vdda_mv(&self, vref: u16) -> u32 {
        hw::adc::vdda_mv(vref)
    }

    /// Phase C and the star are no longer scanned (E041): zeros.
    #[inline(always)]
    fn comp_inputs(&self) -> (u16, u16) {
        (0, 0)
    }

    #[inline(always)]
    fn pwm_counter(&self) -> u32 {
        hw::pwm::counter()
    }

    fn gates_to_timer<G: Drives>(&mut self, _g: &mut Gates<G>) {
        roots::gates_to_timer();
    }

    fn all_phases_pwm<G: Drives>(&mut self, _g: &mut Gates<G>) {
        hw::pwm::all_phases_pwm();
    }

    fn set_compares<G: Drives>(&mut self, _g: &mut Gates<G>, logical: [u32; 3]) {
        if logical == [0; 3] {
            roots::set_compares_wired(logical);
        } else {
            powered_write(|| roots::set_compares_wired(logical));
        }
    }

    #[inline(always)]
    fn apply_plan<G: Drives>(&mut self, _g: &mut Gates<G>, plan: &Plan) {
        powered_write(|| hw::pwm::apply_plan(plan));
    }

    fn moe_on<G: Drives>(&mut self, _g: &mut Gates<G>) {
        powered_write(hw::pwm::moe_on);
    }

    fn set_period(&mut self, period: u32) {
        hw::pwm::set_period(period);
    }

    fn float_all(&mut self) {
        hw::pwm::float_all();
    }

    fn safe_off<G>(&mut self, g: Gates<G>) -> Gates<Stopped> {
        safe_off(&mut Drv8304);
        g.into_stopped()
    }

    #[inline(always)]
    fn comp_mask(&mut self) {
        roots::comp_exti_mask();
    }

    #[inline(always)]
    fn comp_arm(&mut self, step: Step) {
        roots::comp_exti_arm(step);
    }

    #[inline(always)]
    fn comp_select(&mut self, step: Step) {
        roots::comp2_select_floating(step);
    }

    #[inline(always)]
    fn comp_level(&self) -> bool {
        hw::comp::level()
    }

    fn comp_hysteresis(&mut self, hyst: u8) {
        hw::comp::set_hysteresis(hyst);
    }

    fn comp_run_hysteresis(&mut self) {
        hw::comp::set_hysteresis(roots::COMP2_HYST);
    }

    #[inline(always)]
    fn edge_rising(&self, step: Step) -> bool {
        roots::edge_is_rising(step)
    }

    fn reset_roots(&mut self) {
        S.drv().early.store(0, Ordering::Relaxed);
        S.drv().unstable.store(0, Ordering::Relaxed);
        S.drv().defers.store(0, Ordering::Relaxed);
        let _ = S.drv().rate.lock(|r| *r = firmware50::rate::Rate::new());
        let _ = S.det().rate.lock(|r| *r = firmware50::rate::Rate::new());
        S.comp().storm.store(false, Ordering::Relaxed);
        S.comp().overrun.store(false, Ordering::Relaxed);
        S.det().cap_armed.store(false, Ordering::Relaxed);
        S.comp().call_max_us.store(0, Ordering::Relaxed);
        S.comp().overrun_inject_us.store(0, Ordering::Relaxed);
    }

    fn guard_arm(&mut self) {
        self.pace.listen();
        roots::guard_arm();
    }

    #[inline(always)]
    fn guard_reason(&self) -> u32 {
        S.guard().reason.load(Ordering::Relaxed)
    }

    #[inline(always)]
    fn storm(&self) -> bool {
        S.comp().storm.load(Ordering::Relaxed)
    }

    #[inline(always)]
    fn adc_ovr(&self) -> u32 {
        S.guard().adc_ovr.load(Ordering::Relaxed)
    }

    fn late_arms(&self) -> u32 {
        S.det().late_arms.load(Ordering::Relaxed)
    }

    fn unstable_count(&self) -> u32 {
        roots::det_counts().2
    }

    fn wait_hist(&self) -> [u32; 8] {
        core::array::from_fn(|i| S.det().wait_hist[i].load(Ordering::Relaxed))
    }

    fn left_hist(&self) -> [u32; 8] {
        core::array::from_fn(|i| S.det().left_hist[i].load(Ordering::Relaxed))
    }

    #[inline(always)]
    fn blank_latched(&self) -> u32 {
        S.com().blank_latched.load(Ordering::Relaxed)
    }

    #[inline(always)]
    fn overrun(&self) -> bool {
        S.comp().overrun.load(Ordering::Relaxed)
    }

    #[inline(always)]
    fn cap_armed(&self) -> bool {
        S.det().cap_armed.load(Ordering::Relaxed)
    }

    fn arm_cap(&mut self) {
        S.det().cap_armed.store(true, Ordering::Relaxed);
    }

    fn drv_begin(&mut self, step: Step) {
        let d = S.drv();
        d.step.store(u32::from(step.get()), Ordering::Relaxed);
        d.epoch.store(0, Ordering::Relaxed);
        d.deferred.store(false, Ordering::Relaxed);
        d.last_raw.store(u32::from(hw::clock::raw()), Ordering::Relaxed);
        d.sector_raw.store(u32::from(hw::clock::raw()), Ordering::Relaxed);
        self.drv_seq_seen = d.acc_seq.load(Ordering::Relaxed);
        // Mux first, then select the edge and clear pending (`command()`).
        roots::comp2_select_floating(step);
        d.active.store(true, Ordering::Release);
        roots::comp_exti_arm(step);
    }

    fn drv_advance(&mut self, step: Step, epoch: u32) {
        roots::comp_exti_mask();
        let d = S.drv();
        d.deferred.store(false, Ordering::Relaxed);
        d.step.store(u32::from(step.get()), Ordering::Relaxed);
        d.epoch.store(epoch, Ordering::Relaxed);
        d.sector_raw.store(u32::from(hw::clock::raw()), Ordering::Relaxed);
    }

    fn drv_poll(&mut self) -> Option<DrivenAccept> {
        let d = S.drv();
        let s = d.acc_seq.load(Ordering::Relaxed);
        if s == self.drv_seq_seen {
            return None;
        }
        self.drv_seq_seen = s;
        Some(DrivenAccept {
            raw: d.acc_raw.load(Ordering::Relaxed) as u16,
            epoch: d.acc_epoch.load(Ordering::Relaxed) as u16,
            step: Step::new_clamped(d.acc_step.load(Ordering::Relaxed) as u8),
            interval_us: d.acc_interval.load(Ordering::Relaxed),
            position_us: d.acc_pos.load(Ordering::Relaxed) as u16,
        })
    }

    /// The reference's 20 kHz `resume_deferred`, inside one critical section
    /// so the line state cannot change between the check and the pend.
    fn drv_resume_deferred(&mut self, step: Step) -> bool {
        let d = S.drv();
        if !d.deferred.load(Ordering::Relaxed) {
            return false;
        }
        let count = u32::from(hw::clock::raw().wrapping_sub(d.last_raw.load(Ordering::Relaxed) as u16));
        if count <= DRV_GATE_US {
            return false;
        }
        cortex_m::interrupt::free(|_| {
            d.deferred.store(false, Ordering::Relaxed);
            let post = hw::comp::level() == roots::edge_is_rising(step);
            hw::comp::clear_pending();
            if !roots::comp_resume_powered() {
                return false;
            }
            if post {
                hw::comp::pend();
            }
            post
        })
    }

    fn drv_end(&mut self) {
        S.drv().active.store(false, Ordering::Relaxed);
        roots::comp_exti_mask();
    }

    fn det_install(&mut self, zc: ZeroCross, seed_us: u32, raw: u16, step: Step, advance: u32) {
        let det = S.det();
        // Seed the published pair from the estimator being installed, so the
        // handover commutation reads the same values the old borrow would have
        // (step 6a).
        det.accept_avg.store(zc.average_interval(), Ordering::Relaxed);
        det.accept_blank.store(zc.blanking(), Ordering::Relaxed);
        let _ = det.zc.lock(|z| *z = Some(zc));
        // All six slots at the seed, as the reference seeds `interval_hist`.
        let _ = S.com().six.lock(|six| *six = SixSlot::seeded(seed_us));
        det.sector_start_raw.store(u32::from(raw), Ordering::Relaxed);
        // `accept_raw` too: COM phase 1 measures the blanking floor's `since`
        // from it, so a handover left it reading a stale or zero stamp and the
        // first commutation's floor was computed from garbage. Found by the
        // pre-run review of step 6b (E167); it predates the restructure.
        det.accept_raw.store(u32::from(raw), Ordering::Relaxed);
        self.det_seq_seen = det.accept_seq.load(Ordering::Relaxed);
        det.step.store(u32::from(step.get()), Ordering::Relaxed);
        det.advance.store(advance, Ordering::Relaxed);
        let _ = det.rate.lock(|r| *r = firmware50::rate::Rate::new());
        // The same refusal as `com_handover`: this store re-arms the detector,
        // and `guard_trip` has just cleared it. Installing over a latched stop
        // would put COMP back in service after the bridge was de-energised
        // (E181 SS3.2). Refusing leaves `active` false, which every root reads.
        // The two counter resets are inside the section for the same reason as
        // COM's (E186 SS2): a refused install must not erase the tripping
        // run's `late_arms` and `spent_max`.
        cortex_m::interrupt::free(|_| {
            if !roots::guard_latched() {
                // **These three belong inside the guarded section too.** They
                // were reset above it, so an install refused because the guard
                // had already latched still zeroed them -- blanking the margin
                // and the re-base count on exactly the trip they exist to
                // explain. `rebase` carried that flaw before E208 made it
                // visible; both reviews called this the cheapest necessary fix
                // (E209 SS5, E210 SS2).
                det.rebase.store(0, Ordering::Relaxed);
                det.ci_min_p1.store(0, Ordering::Relaxed);
                det.thin.store(0, Ordering::Relaxed);
                det.spent_max.store(0, Ordering::Relaxed);
                det.late_arms.store(0, Ordering::Relaxed);
                det.ci_at_late.store(0, Ordering::Relaxed);
                det.spent_at_late.store(0, Ordering::Relaxed);
                // E315: per-run like every counter above it. A histogram that
                // accumulated across runs would report the ladder, not the run
                // -- the same defect `rebase` had before E208.
                let mut i = 0;
                while i < 8 {
                    det.wait_hist[i].store(0, Ordering::Relaxed);
                    det.left_hist[i].store(0, Ordering::Relaxed);
                    i += 1;
                }
                det.active.store(true, Ordering::Release);
            }
        });
    }

    fn com_handover(&mut self, duty: u16, period: u32, step: Step, commit_us: u32) {
        roots::com_publish_plans(duty, period);
        let com = S.com();
        // **Releasing the stop latch is the one operation that can undo a
        // stop, so it is refused if the guard has already tripped, and the
        // release and the arm are atomic with respect to the guard.**
        //
        // Before E182 this stretch was unconditional: a trip landing anywhere
        // between `Handover::lock`'s `guard_reason` check and here was erased
        // -- the latch cleared, `active` set back to true, TIM16 unmasked and
        // the timer armed, all *after* `guard_trip` had dropped MOE and EN.
        // The exposure was one foreground pass (~150 us) before `pass` saw the
        // still-latched reason, and inside it the stop was lost. Found by the
        // pre-run review of the 50% cohort (E181 SS3.2).
        //
        // A refusal needs no recovery path of its own: `active` stays false, so
        // `com_root` and every `com_arm` refuse, and the next `Ctx::pass` reads
        // the latched `guard_reason` and stops the run with it.
        let now = self.tick_clock();
        cortex_m::interrupt::free(|_| {
            if roots::guard_latched() {
                return;
            }
            // The counter resets live *inside* the guarded section (E186
            // SS2): they used to run unconditionally, so a trip landing in
            // this stretch was correctly refused and then reported with
            // `com_late_max_us=0`, `count=0`, `blank_latched=0` -- the
            // tripping run's own diagnostics erased. `late_arms` is a field
            // this campaign has already been burned by mis-quoting.
            com.step.store(u32::from(step.get()), Ordering::Relaxed);
            com.count.store(0, Ordering::Relaxed);
            com.late_max.store(0, Ordering::Relaxed);
            com.blank_arms.store(0, Ordering::Relaxed);
            // The preemption counters too: they were boot-cumulative while
            // every denominator restarted here, so the ratio was not apples
            // to apples (E170).
            com.preempts.store(0, Ordering::Relaxed);
            com.arm_preempts.store(0, Ordering::Relaxed);
            com.blank_latched.store(0, Ordering::Relaxed);
            com.stopped.store(false, Ordering::Relaxed);
            com.active.store(true, Ordering::Release);
            roots::guard_arm_tracking();
            hw::nvic::unpend(stm32::Interrupt::TIM16);
            hw::nvic::unmask(stm32::Interrupt::TIM16);
            let remaining = commit_us.wrapping_sub(now);
            roots::com_arm(if remaining < u32::MAX / 2 { remaining } else { 1 }, 1);
        });
    }

    #[inline(always)]
    fn det_poll(&mut self) -> Option<firmware50::run::accepted::Accepted> {
        firmware50::run::accepted::observe(
            &mut self.det_seq_seen,
            || S.det().accept_seq.load(Ordering::Relaxed),
            || S.det().accept_raw.load(Ordering::Relaxed) as u16,
        )
    }

    fn det_average(&self) -> Option<u32> {
        roots::det_average_interval()
    }

    #[inline(always)]
    fn com_step(&self) -> Step {
        Step::new_clamped(S.com().step.load(Ordering::Relaxed) as u8)
    }

    #[inline(always)]
    fn com_idle(&self) -> bool {
        S.com().phase.load(Ordering::Relaxed) == 0
    }

    #[inline(always)]
    fn com_count(&self) -> u32 {
        S.com().count.load(Ordering::Relaxed)
    }

    /// Transcribed from the qualified image's `bench-running-level-revisit`
    /// poll: sample the inputs and pend inside one critical section.
    fn revisit(&mut self, step: Step) -> bool {
        cortex_m::interrupt::free(|_| {
            if !firmware50::revisit::sector_ready(
                step.get(),
                S.det().step.load(Ordering::Relaxed),
                S.com().phase.load(Ordering::Relaxed),
            ) {
                return false;
            }
            let now_raw = hw::clock::raw();
            let start = S.det().sector_start_raw.load(Ordering::Relaxed) as u16;
            let v = firmware50::revisit::Inputs {
                closed_loop: S.det().active.load(Ordering::Relaxed),
                line_live: hw::comp::line_live(),
                pending: hw::comp::pending(),
                average_us: roots::det_average_interval().unwrap_or(0),
                elapsed_us: u32::from(now_raw.wrapping_sub(start)),
                post_level: hw::comp::level() == roots::edge_is_rising(step),
                already_retried: false,
            };
            let ok = firmware50::revisit::admit(v);
            if ok {
                hw::comp::pend();
            }
            ok
        })
    }

    fn publish_plans(&mut self, duty: u16, period: u32, cap: u16) {
        roots::com_publish_plans_capped(duty, period, cap);
    }

    fn set_advance(&mut self, advance: u32) {
        S.det().advance.store(advance, Ordering::Relaxed);
    }

    fn det_release(&mut self) {
        S.det().active.store(false, Ordering::Relaxed);
    }

    fn take_back(&mut self) {
        // The driven observer too: a run that stops inside the driven stage
        // would otherwise leave `ADC_COMP` deciding after the bridge is down.
        S.drv().active.store(false, Ordering::Relaxed);
        S.drv().deferred.store(false, Ordering::Relaxed);
        S.com().active.store(false, Ordering::Relaxed);
        roots::com_stop();
        hw::nvic::mask(stm32::Interrupt::TIM16);
        roots::guard_disarm();
        self.pace.unlisten();
    }

    fn inject(&mut self, kind: Inject) {
        match kind {
            // The COMP root stops deciding; nothing is accepted.
            Inject::Tracking => S.det().active.store(false, Ordering::Relaxed),
            // E355: force the counters `states.rs:278-282` polls each closed
            // pass. Deliberately the same atomics the protection reads, so the
            // protection itself is untouched -- what is demonstrated is the
            // stop path, not the physics that would normally set them.
            Inject::LateArm => S.det().late_arms.store(1, Ordering::Relaxed),
            Inject::BlankLatched => S.com().blank_latched.store(1, Ordering::Relaxed),
            Inject::TickGap => cortex_m::interrupt::free(|_| {
                let t0 = hw::clock::raw();
                while hw::clock::raw().wrapping_sub(t0) < firmware50::run::policy::INJECT_STALL_US {}
            }),
            // Stall DMA1 channel 1 (the ADC's); resynchronised after the run.
            Inject::FeedbackStale => hw::adc::stall_dma(),
            // Both ends of the shared nFAULT node are open-drain: benign.
            Inject::Driver => hw::gpio::nfault_inject_assert(),
            Inject::Overrun => S.comp().overrun_inject_us.store(60, Ordering::Relaxed),
            // Real `ADC_COMP` entries 8 µs apart through the real handler and
            // rate latch (the reference's `RATESTOP`), until the unchanged
            // 64/ms cut trips (bounded at 400). Each is unmasked first (E127):
            // a pend while the root holds its NVIC line masked -- most of a
            // 25% sector -- collapses into one entry, and 80 plain pends
            // peaked at 63/ms there.
            Inject::Storm => {
                let mut i = 0;
                while i < 400 && !S.comp().storm.load(Ordering::Relaxed) {
                    hw::nvic::unmask(stm32::Interrupt::ADC_COMP);
                    hw::comp::pend();
                    let t0 = hw::clock::raw();
                    while hw::clock::raw().wrapping_sub(t0) < 8 {}
                    i += 1;
                }
            }
            // Interrupts stay enabled; only the foreground stalls, unfed.
            Inject::Watchdog => {
                let mut ms = 0u32;
                while ms < 100 {
                    let t0 = hw::clock::raw();
                    while hw::clock::raw().wrapping_sub(t0) < 1_000 {}
                    ms += 1;
                }
            }
            // The library does these itself (plans and the current meter).
            Inject::Sag | Inject::AverageCurrent => {}
        }
    }

    fn undo_inject(&mut self, kind: Inject) {
        match kind {
            Inject::Driver => hw::gpio::nfault_inject_release(),
            // Re-enabling the channel alone resumes it out of step with the
            // scan (see `hw::adc::resync`).
            Inject::FeedbackStale => hw::adc::resync(),
            _ => {}
        }
    }

    fn roots_record(&mut self) -> Roots {
        let (zc_accepted, too_early, unstable) = roots::det_counts();
        let (det_peak, storm) = S.det().rate.lock(|r| (r.peak(), r.failed())).unwrap_or((0, false));
        Roots {
            zc_accepted,
            too_early,
            unstable,
            spent_max_us: S.det().spent_max.load(Ordering::Relaxed),
            late_arms: S.det().late_arms.load(Ordering::Relaxed),
            // E314: the estimator value and spend AT the first latch, captured
            // in the ISR rather than read here -- `ci_us` below is sampled at
            // report-build time, after the stop, so a low reading there is
            // confounded with "an abrupt stop reads lower".
            ci_at_late: S.det().ci_at_late.load(Ordering::Relaxed),
            spent_at_late: S.det().spent_at_late.load(Ordering::Relaxed),
            // E208: the causal side of the late arm. `margin_min_p1` is stored
            // plus one so that 0 means "no acceptance seen".
            ci_min_us: S.det().ci_min_p1.load(Ordering::Relaxed).saturating_sub(1),
            thin_count: S.det().thin.load(Ordering::Relaxed),
            wait_hist: core::array::from_fn(|i| S.det().wait_hist[i].load(Ordering::Relaxed)),
            left_hist: core::array::from_fn(|i| S.det().left_hist[i].load(Ordering::Relaxed)),
            // The hold-window pair is a foreground subtraction against the hold
            // mark, done in `run::mod` where the mark lives (E212's pattern).
            wait_hist_hold: [0; 8],
            left_hist_hold: [0; 8],
            hold_unstable: 0,
            rebase: S.det().rebase.load(Ordering::Relaxed),
            com_preempts: S.com().preempts.load(Ordering::Relaxed),
            com_arm_preempts: S.com().arm_preempts.load(Ordering::Relaxed),
            drv_early: S.drv().early.load(Ordering::Relaxed),
            drv_unstable: S.drv().unstable.load(Ordering::Relaxed),
            drv_defers: S.drv().defers.load(Ordering::Relaxed),
            drv_peak: u32::from(S.drv().rate.lock(|r| r.peak()).unwrap_or(0)),
            det_peak: u32::from(det_peak),
            storm,
            storm_step: S.comp().storm_step.swap(0, Ordering::Relaxed),
            cap_armed: S.det().cap_armed.load(Ordering::Relaxed),
            comp_call_max_us: S.comp().call_max_us.load(Ordering::Relaxed),
            overrun: S.comp().overrun.load(Ordering::Relaxed),
            blank_arms: S.com().blank_arms.load(Ordering::Relaxed),
            blank_latched: S.com().blank_latched.load(Ordering::Relaxed),
            com_count: S.com().count.load(Ordering::Relaxed),
            com_late_max_us: S.com().late_max.load(Ordering::Relaxed),
        }
    }

    fn guard_record(&mut self, loop_iters_closed: u32, loop_gap_max_us: u32, acquire_us: u32) -> GuardRecord {
        let watch = S
            .guard()
            .watch
            .lock(|w| *w)
            .unwrap_or(firmware50::tracking::EventWatch::new(
                0,
                firmware50::protection::EVENT_MIN_US,
                firmware50::tracking::EVENT_MAX_US,
            ));
        let (fast_events, fast_min_us) = watch.fast_events();
        GuardRecord {
            reason: S.guard().reason.load(Ordering::Relaxed),
            ticks: S.guard().ticks.load(Ordering::Relaxed),
            gap_max_us: S.guard().gap_max.load(Ordering::Relaxed),
            track_fault: match watch.fault() {
                None => 0,
                Some(firmware50::tracking::Fault::Stale) => 1,
                Some(firmware50::tracking::Fault::TooFast) => 2,
                Some(firmware50::tracking::Fault::SectorOrder) => 3,
            },
            track_max_us: watch.max_interval(),
            fast_events,
            fast_min_us,
            loop_iters_closed,
            loop_gap_max_us,
            acquire_us,
        }
    }
}

// ---------------------------------------------------------------------------
// Bring-up
// ---------------------------------------------------------------------------

/// Everything `main` needs before the shell: clocks, pins (gates and ENABLE
/// low first), USART3, the ADC and its DMA, the timebase, TIM1, the COM
/// one-shot, COMP2 and the watchdog. `None` only if USART3 will not start.
pub fn init(dp: stm32::Peripherals) -> Option<(Board, bool)> {
    // HSI16 -> PLL -> 64 MHz, via the HAL (not a hand-written FLASH_ACR store,
    // which once cleared DBG_SWEN and disabled SWD).
    let mut rcc = dp.RCC.freeze(Config::pll());
    // Flash prefetch on (E135): sequential fetches stop stalling on the two
    // wait states. E133 showed the speed-up alone pushes the comparator's
    // chatter past the storm cut; E134's blank removes that chatter first.
    // The banner reports the readback (`prefetch=`).
    let _ = hw::system::flash_prefetch_enable();
    let port_a = dp.GPIOA.split(&mut rcc);
    let port_b = dp.GPIOB.split(&mut rcc);
    let port_c = dp.GPIOC.split(&mut rcc);
    let port_d = dp.GPIOD.split(&mut rcc);
    // ENABLE low before anything else can energize the bridge.
    let _en = port_d.pd1.into_push_pull_output();
    roots::enable_set(false);
    // nFAULT is open-drain: an internal pull-up so a floating input cannot
    // read healthy and silently disable the fastest protection.
    let _nfault = port_b.pb14.into_pull_up_input();
    let mut led = port_b.pb5.into_push_pull_output();
    led.set_low().ok();
    // Gate pins: outputs, driven low, until preflight passes.
    let _ = port_a.pa7.into_push_pull_output();
    let _ = port_a.pa8.into_push_pull_output();
    let _ = port_a.pa9.into_push_pull_output();
    let _ = port_a.pa10.into_push_pull_output();
    let _ = port_b.pb0.into_push_pull_output();
    let _ = port_b.pb1.into_push_pull_output();
    roots::gates_low_now();
    // USART3 on PC10/PC11 (AF0), a "basic" instance: `BasicConfig`.
    let uart = dp
        .USART3
        .usart(
            (port_c.pc10, port_c.pc11),
            BasicConfig::default().baudrate(115_200.bps()),
            &mut rcc,
        )
        .ok()?;
    let adc_ok = hw::adc::init(&mut rcc);
    let mut clock = dp.TIM17.timer(&mut rcc);
    clock.start(65_535.micros());
    hw::clock::init_1mhz();
    let mut pace = dp.TIM6.timer(&mut rcc);
    // 101 us = 9900.99 Hz: deliberately not the 10 kHz carrier's period.
    pace.start(101.micros());
    if adc_ok {
        hw::adc::dma_start(&mut rcc);
    }
    hw::pwm::init(&mut rcc, STARTUP_TICKS, DTG);
    roots::tim16_init(&mut rcc);
    roots::comp2_init();
    // 50 ms; the HAL's reload is computed for the wrong clock (E080), see
    // `hw::system::iwdg_set_timeout`.
    let dog = dp.IWDG.constrain();
    let _ = hw::system::iwdg_set_timeout(IWDG_RELOAD);
    let board = Board {
        uart,
        clock,
        pace,
        adc_seq_seen: 0,
        adc_last_us: 0,
        drv_seq_seen: 0,
        det_seq_seen: 0,
        _analog: AnalogPins {
            _pa0: port_a.pa0.into_analog(),
            _pa1: port_a.pa1.into_analog(),
            _pa4: port_a.pa4.into_analog(),
            _pa6: port_a.pa6.into_analog(),
            _pa2: port_a.pa2.into_analog(),
            _pa3: port_a.pa3.into_analog(),
            // PB3/PB7 reach COMP2 only, and must be analog for it to see
            // them (their reset state limited the loop to phase C).
            _pb3: port_b.pb3.into_analog(),
            _pb7: port_b.pb7.into_analog(),
        },
        led,
        clock_us: 0,
        last_cnt: 0,
        rx_overruns: 0,
        dog,
    };
    Some((board, adc_ok))
}

/// The boot banner, the reset cause (E080) and a preflight readback.
pub fn banner(board: &mut Board, adc_ok: bool) {
    board.say("\r\nfirmware50 shell-pwm open-loop, protections live (HAL)\r\n");
    board.kv("carrier_hz", 64_000_000 / STARTUP_TICKS);
    board.kv("control_hz", CONTROL_HZ);
    board.kv("adc_hz", 9901);
    board.kv("SCRIPT_TICKS", SCRIPT_TICKS);
    board.kv("envelope_min", u32::from(ENVELOPE_MIN));
    board.kv("envelope_max", u32::from(ENVELOPE_MAX));
    board.kv("envelope_step", u32::from(ENVELOPE_STEP));
    board.kv("raw_limit", RAW_LIMIT);
    board.kv("adc_ok", u32::from(adc_ok));
    board.kv("prefetch", u32::from(hw::system::flash_prefetch_on()));
    let rc = hw::system::take_reset_cause();
    board.say("\r\nRESETCAUSE ");
    board.kv("iwdg", u32::from(rc.iwdg));
    board.kv("wwdg", u32::from(rc.wwdg));
    board.kv("lpwr", u32::from(rc.lpwr));
    board.kv("sft", u32::from(rc.sft));
    board.kv("pwr", u32::from(rc.pwr));
    board.kv("pin", u32::from(rc.pin));
    board.kv("obl", u32::from(rc.obl));
    board.say("\r\n");
    let p = board.preflight();
    firmware50::run::measure::say_preflight(&p, board);
    board.say("\r\nsend '?' for commands\r\n");
    board.tx_flush();
}

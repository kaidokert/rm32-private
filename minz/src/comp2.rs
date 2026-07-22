//! COMP2 BEMF comparator init (STM32L431 / Vimdrones L431).
//!
//! Single-phase observation for the bench tester: INP+ on PB4
//! (virtual neutral / common), INM− switchable across the three
//! motor phases. The label-to-pin mapping follows the textbook
//! six-step BLDC convention (phase A floats at sectors 2 and 5, B
//! at 1 and 4, C at 0 and 3) rather than the Vimdrones board's
//! silkscreen — see the [`ObservedPhase`] doc table.
//! No EXTI, no interrupts — main polls [`value`] to see what the
//! comparator says right now.
//!
//! `stm32l4xx-hal` has no comparator abstraction (no `comp.rs` in the
//! HAL), so this module sits at the PAC level. Every field is set via
//! its typed accessor — no raw `bits()` writes for the whole register
//! — which gives us a free compile-time check that bit positions and
//! widths match the SVD.
//!
//! One L4 quirk worth knowing: writes to `COMP_CSR` are silently
//! dropped when the SYSCFG peripheral clock is gated (the two share
//! a clock domain). `init_phase_a` enables `SYSCFG` first via the
//! HAL's `Enable` trait.

use crate::hal::gpio::Analog;
use crate::hal::gpio::gpioa::{PA4, PA5};
use crate::hal::gpio::gpiob::{PB4, PB7};
use crate::hal::rcc::{APB2, Enable};
use crate::hal::stm32::{COMP, EXTI, SYSCFG};

/// Which phase's BEMF pin is currently routed to COMP2's INM− input.
///
/// Pin mapping follows the **textbook 6-step BLDC convention** (phase
/// A floats at sectors 2 & 5, B at 1 & 4, C at 0 & 3) rather than the
/// Vimdrones board's silkscreen — the silkscreen labels PB7 "phase A"
/// but in the standard convention that pad is phase C, so we swap.
///
/// Encodings on L431 COMP2 (RM0394 22.7.3): all four pins use
/// `INMSEL=0b111` (IO2..5 family), distinguished by the 2-bit
/// `INMESEL` extension field.
///
/// | Phase | Pin  | COMP2 input | INMSEL | INMESEL | Floats at sectors |
/// |-------|------|-------------|--------|---------|-------------------|
/// | A     | PA4  | IO4         | 0b111  | 0b10    | 2, 5              |
/// | B     | PA5  | IO5         | 0b111  | 0b11    | 1, 4              |
/// | C     | PB7  | IO2         | 0b111  | 0b00    | 0, 3              |
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum ObservedPhase {
    A,
    B,
    C,
}

impl ObservedPhase {
    /// Returns `(INMSEL, INMESEL)` bit values for this phase.
    fn inm_bits(self) -> (u8, u8) {
        match self {
            Self::A => (0b111, 0b10),
            Self::B => (0b111, 0b11),
            Self::C => (0b111, 0b00),
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Self::A => "A",
            Self::B => "B",
            Self::C => "C",
        }
    }
}

/// Initialise COMP2 for BEMF observation, defaulting to phase A
/// (PA4 in the textbook 6-step convention — see [`ObservedPhase`]).
///
/// Pin map (Vimdrones L431):
///
/// | COMP2 input | Pin | Signal              |
/// |-------------|-----|---------------------|
/// | INP+ (IO1)  | PB4 | virtual neutral     |
/// | INM− (IO4)  | PA4 | phase A BEMF        |
/// | INM− (IO5)  | PA5 | phase B BEMF        |
/// | INM− (IO2)  | PB7 | phase C BEMF        |
///
/// All four pins are consumed in `Analog` typestate so we know they
/// were configured correctly upstream — analog GPIO mode is required
/// for the pads to connect to the comparator's analog input. Only
/// one of PB7/PA5/PA4 is routed at a time; switch with
/// [`set_observed_phase`].
pub fn init(
    _pb4: PB4<Analog>,
    _pb7: PB7<Analog>,
    _pa5: PA5<Analog>,
    _pa4: PA4<Analog>,
    apb2: &mut APB2,
) {
    // L4 quirk: `COMP_CSR` shares its register clock with SYSCFG.
    // Without SYSCFGEN, the writes below would be silently dropped
    // and readback would always return 0.
    SYSCFG::enable(apb2);

    let comp = unsafe { &*COMP::ptr() };

    // RM0394 22.7.3 COMP2_CSR static fields. INMSEL / INMESEL go via
    // `set_observed_phase` and HYST goes via `set_hysteresis` so each
    // runtime-tunable field has a single point of truth.
    //   INPSEL   = 0     → IO1  = PB4 (INP+, virtual neutral)
    //   PWRMODE  = 0b00  → high-speed
    //   POLARITY = 0     → non-inverted (VALUE=1 when INP > INM)
    //   EN       = 1     → enable
    //   WINMODE / BLANKING / BRGEN / SCALEN = 0 (off / unused)
    // INMSEL / INMESEL / HYST default to 0 here; they get
    // overwritten ~µs later by the helper calls below. EXTI is
    // masked at boot so any transient doesn't fire IRQs.
    comp.comp2_csr.write(|w| unsafe {
        w.comp2_en()
            .set_bit()
            .comp2_pwrmode()
            .bits(0b00)
            .comp2_inpsel()
            .bits(0b00)
            .comp2_polarity()
            .clear_bit()
            // BLANKING = 0b000 (off). We investigated routing TIM15
            // OC1 here (0b100) and found that on L4 it gates only
            // COMP_CSR.VALUE — *not* the EXTI line — so it doesn't
            // suppress the COMP IRQ rate. Bench now uses pure software
            // blanking in the COMP ISR (latch a timestamp at every
            // PWM falling edge in TIM1_CC, suppress events within
            // BLANK_US of that timestamp). See `CLAUDE.md`.
            .comp2_blanking()
            .bits(0b000)
            .comp2_winmode()
            .clear_bit()
    });

    set_hysteresis(0);
    // Route INM− to phase A's pin via the shared input-mux helper.
    // Single point of truth for INMSEL + INMESEL bit programming, and
    // its trailing `asm::delay(400)` also covers the post-EN=1 startup
    // settling time (RM0394 22.5.4: < 5 µs in high-speed mode).
    set_observed_phase(ObservedPhase::A);
}

/// Live-switch the COMP2 INM− input to a different phase. Uses
/// `modify` so the surrounding bits (EN, HYST, PWRMODE, POLARITY,
/// INP) are preserved. Caller should mask EXTI around the call to
/// avoid latching a transient edge from the input switchover, and
/// allow ~5 µs settling time afterwards (provided by an in-function
/// `asm::delay`).
pub fn set_observed_phase(phase: ObservedPhase) {
    set_inm(phase);
    cortex_m::asm::delay(400);
}

/// [`set_observed_phase`] minus the blocking settle delay — for ISR
/// use (per-sector mux switching à la AM32's `changeCompInput`). The
/// comparator output is not to be trusted for ~5 µs after the switch;
/// ISR callers must discard early edges by other means (the bench's
/// half-sector time gate covers this with three orders of margin).
///
/// Register race note: this touches the same `COMP2_CSR` as the
/// main-context `set_hysteresis` / `set_observed_phase` — main-side
/// callers must wrap their `modify` in `interrupt::free` once any ISR
/// starts calling this.
#[inline]
pub fn set_inm(phase: ObservedPhase) {
    let comp = unsafe { &*COMP::ptr() };
    let (inmsel, inmesel) = phase.inm_bits();
    comp.comp2_csr
        .modify(|_, w| unsafe { w.comp2_inmsel().bits(inmsel).comp2_inmesel().bits(inmesel) });
}

/// Current COMP2 output bit. Reflects the comparator output with the
/// `POLARITY` bit's inversion applied:
/// - `true`  → if POLARITY=0: INP+ (PB4) above INM− (selected phase pin)
///              if POLARITY=1: INP+ at or below INM−
/// - `false` → the opposite of the above.
#[inline]
pub fn value() -> bool {
    let comp = unsafe { &*COMP::ptr() };
    comp.comp2_csr.read().comp2_value().bit_is_set()
}

/// Live-set the COMP2_CSR.HYST field (2 bits):
///   `0b00` = none, `0b01` = low, `0b10` = medium, `0b11` = high.
/// Higher hysteresis suppresses near-zero noise edges at the cost of
/// adding a deadband around true zero crossings. Used by the bench's
/// `h` key handler.
#[inline]
pub fn set_hysteresis(hyst: u8) {
    let comp = unsafe { &*COMP::ptr() };
    comp.comp2_csr
        .modify(|_, w| unsafe { w.comp2_hyst().bits(hyst & 0b11) });
}

/// Configure EXTI line 22 (COMP2) for both rising and falling edge
/// detection. Leaves the line **masked** at the EXTI controller —
/// caller controls the gate via [`set_exti_enabled`] so counting can
/// be confined to a specific commutation window (e.g. only when the
/// observed phase is floating). The caller is also responsible for
/// unmasking `Interrupt::COMP` at the NVIC.
pub fn configure_exti_both_edges() {
    set_exti_edges(true, true);
    // IMR1.MR22 left in its reset state (0 = masked).
}

/// Live-set which COMP2 EXTI edges trigger the IRQ. Either or both
/// may be `true`; both `false` means no edges trigger (but the line
/// stays masked separately by [`set_exti_enabled`]). Used by the
/// bench's `k` key handler and by the TIM7 ISR for sector-dependent
/// edge modes.
#[inline]
pub fn set_exti_edges(rising: bool, falling: bool) {
    let exti = unsafe { &*EXTI::ptr() };
    exti.rtsr1.modify(|_, w| w.tr22().bit(rising));
    exti.ftsr1.modify(|_, w| w.tr22().bit(falling));
}

/// Mask / unmask EXTI line 22 at the controller.
///
/// `true` → unmasked, edges fire `Interrupt::COMP`.
/// `false` → masked, edges still set `PR1.PR22` but don't reach the
/// NVIC.
///
/// When transitioning from masked → unmasked, the pending bit is
/// cleared first so any edges latched during the masked window
/// (e.g. PWM ringing on a driven phase) are discarded rather than
/// firing the ISR the instant we unmask.
#[inline]
pub fn set_exti_enabled(enabled: bool) {
    let exti = unsafe { &*EXTI::ptr() };
    if enabled {
        exti.pr1.write(|w| w.pr22().set_bit());
    }
    exti.imr1.modify(|_, w| w.mr22().bit(enabled));
}

/// AM32-VERBATIM unmask (2026-07-18): `EXTI->IMR1 |= LINE` with the
/// pending bit PRESERVED - a crossing that arrived during the masked
/// commutation window is SERVICED immediately on unmask instead of
/// discarded. AM32's enableCompInterrupts does exactly this; our
/// clear-first unmask deleted the true crossing whenever it landed
/// inside the mask (the +52 us CL acceptance equilibrium).
#[inline]
pub fn unmask_keep_pending() {
    let exti = unsafe { &*EXTI::ptr() };
    exti.imr1.modify(|_, w| w.mr22().set_bit());
}

/// Ack the EXTI line 22 pending bit. PR1 is write-1-to-clear, so this
/// is a `write` not `modify`. Must be called at the top of the COMP
/// ISR or the IRQ will re-fire forever.
#[inline]
pub fn clear_pending() {
    let exti = unsafe { &*EXTI::ptr() };
    exti.pr1.write(|w| w.pr22().set_bit());
}

/// Software re-pend of the COMP2 EXTI line (SWIER1[22]) — the AM32
/// camp-at-the-gate port. AM32's COMP_IRQHandler does NOT clear the
/// pending flag when its gate is closed but the comparator level
/// already sits post-ZC: the IRQ re-fires until the gate opens and
/// the SAME crossing is then accepted — their gate is a WAIT. Ours
/// acked-then-discarded (~57k gate rejects/run), and a crossed BEMF
/// never edges again → the pre-crossed dead-window class. Re-pending
/// after our storm-safe entry-ack reproduces their semantics.
#[inline]
pub fn sw_repend() {
    let exti = unsafe { &*EXTI::ptr() };
    // SWIER is write-1-to-set; 0s elsewhere are no-ops.
    exti.swier1.write(|w| unsafe { w.bits(1 << 22) });
}

// ===============================================================
// AM32 comparator.c transliterations (used by the am32_clone; kept
// with the peripheral they drive).
// ===============================================================

/// Per-sector floating phase for the AM32 six-step (comparator.c
/// changeCompInput: steps 1/4 -> C, 2/5 -> B... mapped to the minz
/// sector frame 0..5 and this board's textbook phase labels).
pub const AM32_SECTOR_FLOAT_PHASE: [ObservedPhase; 6] = [
    ObservedPhase::C,
    ObservedPhase::B,
    ObservedPhase::A,
    ObservedPhase::C,
    ObservedPhase::B,
    ObservedPhase::A,
];

/// maskPhaseInterrupts (AM32 comparator.c:9-12): clear EXTI IMR +
/// clear the pending flag.
#[inline]
pub fn am32_mask_phase_interrupts() {
    set_exti_enabled(false);
    clear_pending();
}

/// enableCompInterrupts (AM32 comparator.c:14-16): set EXTI IMR,
/// KEEP pending (a crossing latched during the mask is serviced on
/// unmask, not discarded).
#[inline]
pub fn am32_enable_comp_interrupts() {
    unmask_keep_pending();
}

/// changeCompInput (AM32 comparator.c:18-35): mux the floating phase
/// and select the single expected-direction EXTI edge for the sector.
/// `edges_for(3, sector)` is the minz-polarity-correct form of AM32''s
/// `if(rising)` edge select.
#[inline]
pub fn am32_change_comp_input(sector: usize) {
    set_inm(AM32_SECTOR_FLOAT_PHASE[sector]);
    let (re, fe) = minz_core::drive::edges_for(3, sector as u8);
    set_exti_edges(re, fe);
}

/// getBemfState() — main.c:817-852 (L431 `!getCompOutputLevel()` branch,
/// which equals minz `comp2::value()`). Counts when the level matches the
/// direction; a run of bad reads over threshold resets the counter.
#[inline]
pub fn am32_get_bemf_state(drive: &minz_core::am32_loop::Drive) {
    use core::sync::atomic::Ordering;
    let cs = value(); // = !getCompOutputLevel() (main.c:831)
    let rising = drive.rising.load(Ordering::Relaxed);
    // rising: count when current_state; else count when !current_state.
    // Both reduce to `cs == rising` (main.c:833-851).
    let (bemf, bad) = minz_core::am32::bemf_count_step(
        drive.bemf_counter.load(Ordering::Relaxed),
        drive.bad_count.load(Ordering::Relaxed),
        cs == rising,
        minz_core::am32_loop::BAD_COUNT_THRESHOLD,
    );
    drive.bemf_counter.store(bemf, Ordering::Relaxed);
    drive.bad_count.store(bad, Ordering::Relaxed);
}

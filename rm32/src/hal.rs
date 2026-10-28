//! Hardware Abstraction Layer traits.
//!
//! Uses `embedded-hal` 1.0 traits where applicable. ESC-specific traits
//! are defined here for things embedded-hal doesn't cover (comparator,
//! commutation timer, etc; the power stage is `crate::bridge::Bridge`).
//!
//! Implementations are provided by platform crates (rm32_stm32, rm32_std).

/// Re-export embedded-hal traits we use
pub use embedded_hal::delay::DelayNs;
pub use embedded_hal::digital::InputPin;
pub use embedded_hal::pwm::SetDutyCycle;

/// The power stage (gate legs + PWM timer): see [`crate::bridge`].
pub use crate::bridge::Bridge;

/// The gate legs (the half of [`Bridge`] that commutates): AM32 `comStep` / `allOff` / `fullBrake` / `allpwm` /
/// `proportionalBrake`. Semantic methods, not raw leg roles, so harness
/// counters and reverse rotation stay meaningful. Code that only commutates
/// (the commutation-timer path) asks for this much.
pub trait PhaseOutput {
    /// Drive commutation step `step` (1..=6, [`crate::bridge::step_legs`])
    /// with the driven leg complementary or diode
    /// ([`crate::bridge::drive_complementary`], the core's decision); any
    /// other step leaves the outputs unchanged.
    fn com_step(&mut self, step: u8, complementary: bool);
    /// Every leg floating (AM32 `allOff`).
    fn all_off(&mut self);
    /// Every low side on (AM32 `fullBrake`).
    fn full_brake(&mut self);
    /// Every leg driven by PWM (AM32 `allpwm`).
    fn all_pwm(&mut self, complementary: bool);
    /// High sides off, low sides PWM (AM32 `proportionalBrake`).
    fn proportional_brake(&mut self);
    /// RPM pulse output on steps 1 and 4 (debug); boards without one ignore it.
    #[inline(always)]
    fn pulse_toggle(&mut self, _step: u8) {}
}

/// Comparator (BEMF sensing) interface
/// A motor phase.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[repr(u8)]
pub enum Phase {
    A,
    B,
    C,
}

/// The phase left floating (and so sensed for BEMF) in commutation step
/// `step` (AM32 `changeCompInput`): 1|4 -> C, 2|5 -> A, 3|6 -> B, and 0 or
/// 7 -> C as the ports' mux tables did. `Commutation` keeps `step` in
/// 1..=6; larger values wrap modulo 8.
///
/// A branch-free table: a `match` compiles to a bit-test chain on M0.
#[inline(always)]
pub const fn floating_phase(step: u8) -> Phase {
    const TABLE: [Phase; 8] = [
        Phase::C,
        Phase::C,
        Phase::A,
        Phase::B,
        Phase::C,
        Phase::A,
        Phase::B,
        Phase::C,
    ];
    TABLE[(step & 7) as usize]
}

/// BEMF sensing: the comparator's phase routing and its edge interrupt.
///
/// Routing and interrupt gating are separate calls: the polling path
/// (`old_routine`) re-routes every step while the edge interrupt stays
/// masked.
pub trait BemfSensor {
    /// Route the comparator to `phase` and arm the edge for the motor's
    /// `rising` BEMF. Does not unmask. Board/MCU polarity (AM32
    /// `INVERTED_EXTI`, G431 pin routing) is the port's business.
    fn select_phase(&mut self, phase: Phase, rising: bool);
    fn enable_irq(&mut self);
    fn mask_irq(&mut self);
    /// Comparator output level.
    fn level(&self) -> bool;
    /// One read of the zero-cross persistence filter (`bemf_zero_cross`).
    /// Defaults to `level`; a port overrides it when its inlined read is
    /// much faster than the reference's, so the filter's TIME window
    /// (filter_level reads) matches the reference rather than only its
    /// read count.
    #[inline]
    fn filter_read(&self) -> bool {
        self.level()
    }
    /// The comparator's edge interrupt is pending.
    fn pending(&self) -> bool;
    /// Clear the pending edge.
    fn clear_pending(&mut self);
    /// Re-raise the edge interrupt in software (e.g. a software-trigger register).
    fn repend(&mut self);
}

/// Debug pulse output — toggles a GPIO on commutation for RPM measurement.
pub trait PulseOutput {
    fn toggle(&mut self);
}

/// Interval timer (commutation timing measurement)
pub trait IntervalTimer {
    fn count(&self) -> u32;
    fn set_count(&mut self, val: u32);
}

/// Commutation timer (one-shot for next commutation event)
pub trait ComTimer {
    fn set_and_enable(&mut self, timeout: u16);
    fn disable_interrupt(&mut self);
    fn enable_interrupt(&mut self);
}

/// Bundle of ISR-level motor peripherals for static dispatch.
///
/// Reduces generic parameter count from 5 to 1 in ISR function signatures.
/// Implementors provide the concrete MCU-specific types.
pub trait MotorHal: crate::platform::Platform {
    /// The bridge as a view borrowed from the HAL (a port may build it from
    /// separate peripherals without moving them).
    type Bridge<'a>: Bridge
    where
        Self: 'a;
    type Bemf: BemfSensor;
    type Interval: IntervalTimer;
    type Com: ComTimer;

    fn bridge(&mut self) -> Self::Bridge<'_>;
    fn bemf(&mut self) -> &mut Self::Bemf;
    fn interval(&mut self) -> &mut Self::Interval;
    fn com_timer(&mut self) -> &mut Self::Com;
}

/// Serial telemetry output (KISS protocol)
pub trait TelemetryUart {
    fn send(&mut self, data: &[u8]);
}

/// Bidir DShot output timer prescaler.
///
/// AM32 uses these PSC values for DShot response timing: DShot600 on normal
/// clocks = 0, DShot300 or high-clock DShot600 = 1, high-clock DShot300 = 3.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DshotOutputPrescaler(u16);

impl DshotOutputPrescaler {
    pub const DSHOT600: Self = Self(0);

    pub fn new(value: u16) -> Option<Self> {
        match value {
            0 | 1 | 3 => Some(Self(value)),
            _ => None,
        }
    }

    pub fn raw(self) -> u16 {
        self.0
    }
}

/// Input signal capture (DShot/Servo via DMA)
pub trait InputCapture {
    fn start_capture(&mut self);
    fn start_gcr_output(&mut self);
    fn input_pin_state(&self) -> bool;
    fn set_pull_up(&mut self);
    fn set_pull_down(&mut self);
    fn set_pull_none(&mut self);
    /// Set inverted input polarity (for boards with signal inversion).
    fn set_inverted(&mut self, _inverted: bool) {}
    /// Access the DMA receive buffer (DShot/servo frames).
    fn edges(&self) -> &[u32; 64];
    /// Access the GCR encode buffer (bidirectional DShot telemetry).
    fn gcr_buffer(&mut self) -> &mut [u32; 37];
    /// Whether the input is currently in output mode (bidir DShot TX).
    fn is_output(&self) -> bool;
    /// Set the timer prescaler for bidir DShot output.
    /// Called during protocol detection. DShot600=0, DShot300=1, DShot150=3.
    fn set_output_prescaler(&mut self, prescaler: DshotOutputPrescaler);
}

/// ADC readings (voltage, current, temperature)
pub trait Adc {
    fn start_conversion(&mut self);
    fn raw_voltage(&self) -> u16;
    fn raw_current(&self) -> u16;
    fn raw_temperature(&self) -> u16;
    fn calc_temperature(&self, raw: u16) -> crate::units::DegreesCelsius;
    /// Trigger second ADC conversion (for boards with dual ADC). Default no-op.
    fn start_conversion_2(&mut self) {}
}

/// Flash storage for persistent settings
pub trait Flash {
    fn read(&self, address: u32, buf: &mut [u8]);
    fn write(&mut self, address: u32, data: &[u8]);
}

/// Serial input RX (CRSF protocol)
pub trait SerialInput {
    /// Read available bytes into buffer. Returns number of bytes read.
    fn read_available(&mut self, buf: &mut [u8]) -> usize;
}

/// Emergency hardware safety — all FETs off, no state needed.
/// Used when ISR state is missing or unrecoverable error occurs.
pub trait EmergencyOff {
    /// Force all motor FETs off via direct GPIO writes. Does not require HAL state.
    fn emergency_off();
}

/// System control (IRQ, watchdog, reset)
pub trait System {
    fn reset(&mut self) -> !;
    fn enable_irq(&mut self);
    fn disable_irq(&mut self);
    fn irqs_enabled(&self) -> bool;
    /// Start the independent watchdog with given prescaler and reload values.
    fn start_watchdog(&mut self, prescaler: u8, reload: u16);
    fn reload_watchdog(&mut self);
    fn delay_micros(&mut self, us: u32);
    fn delay_millis(&mut self, ms: u32);
}

#[cfg(test)]
mod tests {
    use super::DshotOutputPrescaler;

    #[test]
    fn dshot_output_prescaler_accepts_supported_values() {
        for value in [0, 1, 3] {
            assert_eq!(DshotOutputPrescaler::new(value).unwrap().raw(), value);
        }
    }

    #[test]
    fn dshot_output_prescaler_rejects_unsupported_values() {
        assert_eq!(DshotOutputPrescaler::new(2), None);
        assert_eq!(DshotOutputPrescaler::new(4), None);
        assert_eq!(DshotOutputPrescaler::new(u16::MAX), None);
    }
}

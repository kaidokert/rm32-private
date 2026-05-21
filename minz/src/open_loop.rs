//! Open-loop three-phase waveforms (120° apart) for bench motor spin-up.

use crate::SYSCLK;

/// Electrical frequency for `open_loop_step_cycles` (Hz). Keep low open-loop to limit slip loss.
pub const OPEN_LOOP_ELECTRICAL_HZ: u32 = 60;

/// Angle steps per electrical revolution (one table entry per degree).
pub const OPEN_LOOP_STEPS_PER_REV: u32 = 360;

/// DWT cycles between angle steps at `OPEN_LOOP_ELECTRICAL_HZ`.
pub const OPEN_LOOP_STEP_CYCLES: u32 =
    SYSCLK.raw() / (OPEN_LOOP_ELECTRICAL_HZ * OPEN_LOOP_STEPS_PER_REV);

/// 360-entry sine table (0–360° → 0–360 duty units). Matches AM32 / rm32 `pwmSin[]`.
static PWM_SIN: [i16; 360] = [
    180, 183, 186, 189, 193, 196, 199, 202, 205, 208, 211, 214, 217, 220, 224, 227, 230, 233, 236,
    239, 242, 245, 247, 250, 253, 256, 259, 262, 265, 267, 270, 273, 275, 278, 281, 283, 286, 288,
    291, 293, 296, 298, 300, 303, 305, 307, 309, 312, 314, 316, 318, 320, 322, 324, 326, 327, 329,
    331, 333, 334, 336, 337, 339, 340, 342, 343, 344, 346, 347, 348, 349, 350, 351, 352, 353, 354,
    355, 355, 356, 357, 357, 358, 358, 359, 359, 359, 360, 360, 360, 360, 360, 360, 360, 360, 360,
    359, 359, 359, 358, 358, 357, 357, 356, 355, 355, 354, 353, 352, 351, 350, 349, 348, 347, 346,
    344, 343, 342, 340, 339, 337, 336, 334, 333, 331, 329, 327, 326, 324, 322, 320, 318, 316, 314,
    312, 309, 307, 305, 303, 300, 298, 296, 293, 291, 288, 286, 283, 281, 278, 275, 273, 270, 267,
    265, 262, 259, 256, 253, 250, 247, 245, 242, 239, 236, 233, 230, 227, 224, 220, 217, 214, 211,
    208, 205, 202, 199, 196, 193, 189, 186, 183, 180, 177, 174, 171, 167, 164, 161, 158, 155, 152,
    149, 146, 143, 140, 136, 133, 130, 127, 124, 121, 118, 115, 113, 110, 107, 104, 101, 98, 95,
    93, 90, 87, 85, 82, 79, 77, 74, 72, 69, 67, 64, 62, 60, 57, 55, 53, 51, 48, 46, 44, 42, 40, 38,
    36, 34, 33, 31, 29, 27, 26, 24, 23, 21, 20, 18, 17, 16, 14, 13, 12, 11, 10, 9, 8, 7, 6, 5, 5,
    4, 3, 3, 2, 2, 1, 1, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 1, 1, 2, 2, 3, 3, 4, 5, 5, 6, 7, 8, 9,
    10, 11, 12, 13, 14, 16, 17, 18, 20, 21, 23, 24, 26, 27, 29, 31, 33, 34, 36, 38, 40, 42, 44, 46,
    48, 51, 53, 55, 57, 60, 62, 64, 67, 69, 72, 74, 77, 79, 82, 85, 87, 90, 93, 95, 98, 101, 104,
    107, 110, 113, 115, 118, 121, 124, 127, 130, 133, 136, 140, 143, 146, 149, 152, 155, 158, 161,
    164, 167, 171, 174, 177,
];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Waveform {
    Sine,
    /// True 6-step BLDC: 2 phases driven, 1 floating (Hi-Z) per sector.
    /// Not a continuous 3-phase waveform like the other variants — use
    /// [`six_step_sector`] + a timer driver that can tri-state channels.
    SixStep,
}

/// Advance electrical angle (0–359°), forward = decreasing index (rm32 motor convention).
#[inline]
pub fn advance_angle(angle: u16, forward: bool) -> u16 {
    if forward {
        if angle == 0 { 359 } else { angle - 1 }
    } else if angle >= 359 {
        0
    } else {
        angle + 1
    }
}

/// Map table entry to centered PWM duty: table 180 = neutral (ARR/2 envelope), 0/360 = peaks.
#[inline]
fn table_duty(table_val: i16, arr: u16, amplitude_pct: u16) -> u16 {
    let half = arr as u32 * amplitude_pct as u32 / 100 / 2;
    let centered = (table_val as i32 - 180) * half as i32 / 180;
    (half as i32 + centered).clamp(0, arr as i32) as u16
}

/// Sinusoidal PWM: phases at θ, θ+120°, θ+240° (centered around neutral duty).
pub fn sine_duties(angle: u16, arr: u16, amplitude_pct: u16) -> (u16, u16, u16) {
    let a = angle as usize % 360;
    let b = (angle as usize + 120) % 360;
    let c = (angle as usize + 240) % 360;
    (
        table_duty(PWM_SIN[a], arr, amplitude_pct),
        table_duty(PWM_SIN[b], arr, amplitude_pct),
        table_duty(PWM_SIN[c], arr, amplitude_pct),
    )
}

pub fn duties(waveform: Waveform, angle: u16, arr: u16, amplitude_pct: u16) -> (u16, u16, u16) {
    match waveform {
        Waveform::Sine => sine_duties(angle, arr, amplitude_pct),
        // SixStep is not a continuous duty triple — caller handles it.
        Waveform::SixStep => (0, 0, 0),
    }
}

/// Map electrical angle (0..360) → 6-step BLDC sector index (0..5).
/// 60° per sector; `(angle / 60) clamped to 0..5`.
#[inline]
pub fn six_step_sector(angle: u16) -> u8 {
    (angle / 60).min(5) as u8
}

/// 6-step high-side PWM compare value: `amplitude_pct` percent of `arr`.
/// Unlike sine where amplitude is half-swing around `arr/2`, in 6-step
/// the high-side FET PWMs from 0% (off) to `amplitude_pct`% (on),
/// while the low-side phase sits at 0 (low FET always on via the
/// complementary), so the line-to-line average is `amplitude_pct`% of
/// the bus directly.
#[inline]
pub fn six_step_duty(arr: u16, amplitude_pct: u16) -> u16 {
    (arr as u32 * amplitude_pct as u32 / 100).min(arr as u32) as u16
}

//! M0-friendly sine PWM scaling. The only non-power-of-two division happens
//! when foreground prepares a new duty; the periodic timer uses shifts/mults.
pub const fn amplitude(arr: u32, duty_tenths: u32) -> u32 {
    (arr + 1) * duty_tenths / 1000
}
/// Exact floor(x/255) over the bounded PWM product domain (x<=489_600).
pub const fn div255(x: u32) -> u32 {
    let folded = x + 1 + (x >> 8);
    (folded + (folded >> 16)) >> 8
}
pub const fn compare(prepared_amplitude: u32, sine: u8) -> u32 {
    div255(prepared_amplitude * sine as u32)
}
/// For 10kHz/6400 ticks, round(ccr*100/6400) = round(ccr/64).
pub const fn on_us_10k(ccr: u32) -> u16 {
    ((ccr + 32) >> 6) as u16
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn div255_is_exact_for_whole_pwm_domain() {
        for x in 0..=489_600 {
            assert_eq!(div255(x), x / 255, "x={x}");
        }
    }
    #[test]
    fn prepared_wave_is_never_higher_and_at_most_one_tick_lower() {
        let arr = 6399;
        for duty in 0..=300 {
            let prepared = amplitude(arr, duty);
            for sine in 0..=255u8 {
                let old = (arr + 1) * duty * sine as u32 / (1000 * 255);
                let new = compare(prepared, sine);
                assert!(new <= old && old - new <= 1, "duty={duty} sine={sine}");
                assert_eq!(
                    on_us_10k(old),
                    ((old * 100 + (arr + 1) / 2) / (arr + 1)) as u16
                );
            }
        }
    }
}

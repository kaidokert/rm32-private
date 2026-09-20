//! Five-channel acquisition snapshot age. Caller samples the clock after
//! copying the raw values and their original acquisition timestamps.
//! The existing 200us setup reserve covers work after this observation.
#[inline(always)]
pub fn age(times: [u16; 5], now: u16) -> Option<u32> {
    const SETUP_US: u32 = 200;
    const MAX_AGE_US: u32 = 1000;
    let oldest = times
        .into_iter()
        .map(|t| now.wrapping_sub(t) as u32)
        .max()
        .unwrap();
    let age = oldest + SETUP_US;
    if age > MAX_AGE_US { None } else { Some(age) }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn exhaustive_clock_wrap_and_exact_boundary() {
        for now in 0..=u16::MAX {
            for oldest in [0u16, 1, 799, 800, 801, 1000, 65535] {
                let times = [now, now.wrapping_sub(oldest), now, now, now];
                assert_eq!(
                    age(times, now),
                    if oldest <= 800 {
                        Some(oldest as u32 + 200)
                    } else {
                        None
                    }
                );
            }
        }
    }
    #[test]
    fn every_channel_can_be_oldest_and_timestamps_are_not_refreshed() {
        for index in 0..5 {
            let mut times = [100u16; 5];
            times[index] = 0;
            assert_eq!(age(times, 800), Some(1000));
            assert_eq!(age(times, 801), None);
        }
    }
}

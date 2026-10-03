//! Pure raw-code bins for a report-only physical phase-current census.
//! Input is already remapped to physical IA/IB/IC by dma_snapshot::logical.
//! Thresholds are raw-code displacements, NOT calibrated amp or SOA guards.

pub const ANY_1500: u8 = 1 << 0;
pub const ANY_1900: u8 = 1 << 1;
pub const IA_1900: u8 = 1 << 2;
pub const IB_1900: u8 = 1 << 3;
pub const IC_1900: u8 = 1 << 4;
pub const EXACT_RAIL: u8 = 1 << 5;

#[inline(always)]
pub fn classify(phase: [u16; 3], zero: [u16; 3]) -> u8 {
    let a = phase[0].abs_diff(zero[0]);
    let b = phase[1].abs_diff(zero[1]);
    let c = phase[2].abs_diff(zero[2]);
    let mut flags = 0;
    if a >= 1500 || b >= 1500 || c >= 1500 {
        flags |= ANY_1500;
    }
    if a >= 1900 || b >= 1900 || c >= 1900 {
        flags |= ANY_1900;
    }
    if a >= 1900 {
        flags |= IA_1900;
    }
    if b >= 1900 {
        flags |= IB_1900;
    }
    if c >= 1900 {
        flags |= IC_1900;
    }
    if phase[0] == 0
        || phase[0] == 4095
        || phase[1] == 0
        || phase[1] == 4095
        || phase[2] == 0
        || phase[2] == 4095
    {
        flags |= EXACT_RAIL;
    }
    flags
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zero_is_per_physical_phase_and_thresholds_are_inclusive() {
        let zero = [2010, 2048, 2060];
        assert_eq!(classify(zero, zero), 0);
        assert_eq!(classify([3510, 2048, 2060], zero), ANY_1500);
        assert_eq!(
            classify([3910, 2048, 2060], zero),
            ANY_1500 | ANY_1900 | IA_1900
        );
        assert_eq!(
            classify([2010, 148, 2060], zero),
            ANY_1500 | ANY_1900 | IB_1900
        );
        assert_eq!(
            classify([2010, 2048, 160], zero),
            ANY_1500 | ANY_1900 | IC_1900
        );
    }

    #[test]
    fn exact_rail_is_independent_of_near_rail_bins() {
        let flags = classify([0, 2048, 4095], [2048; 3]);
        assert_eq!(
            flags & (ANY_1500 | ANY_1900 | IA_1900 | IC_1900 | EXACT_RAIL),
            ANY_1500 | ANY_1900 | IA_1900 | IC_1900 | EXACT_RAIL
        );
        assert_eq!(flags & IB_1900, 0);
        assert_eq!(classify([1, 2048, 4094], [2048; 3]) & EXACT_RAIL, 0);
    }
}

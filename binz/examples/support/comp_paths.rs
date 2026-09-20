//! Classification only; does not read hardware or grant sensing/output authority.
pub const N: usize = 5;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(usize)]
pub enum Path {
    NoGate = 0,
    Closed = 1,
    OpenNoAccept = 2,
    Accepted = 3,
    StoppedOrUnknown = 4,
}
pub fn classify(
    first_count: Option<u32>,
    average: u32,
    accepted_delta: u32,
    stopped: bool,
) -> Path {
    if stopped || accepted_delta > 1 {
        return Path::StoppedOrUnknown;
    }
    let Some(count) = first_count else {
        return if accepted_delta == 0 {
            Path::NoGate
        } else {
            Path::StoppedOrUnknown
        };
    };
    if count <= average >> 1 {
        return if accepted_delta == 0 {
            Path::Closed
        } else {
            Path::StoppedOrUnknown
        };
    }
    if accepted_delta == 1 {
        Path::Accepted
    } else {
        Path::OpenNoAccept
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn strict_reference_gate_boundary() {
        assert_eq!(classify(Some(600), 1200, 0, false), Path::Closed);
        assert_eq!(classify(Some(601), 1200, 0, false), Path::OpenNoAccept);
        assert_eq!(classify(Some(601), 1200, 1, false), Path::Accepted);
        assert_eq!(classify(Some(600), 1201, 0, false), Path::Closed);
        assert_eq!(classify(Some(601), 1201, 1, false), Path::Accepted);
    }
    #[test]
    fn stopped_and_inconsistent_callbacks_are_not_noise_rejections() {
        assert_eq!(classify(Some(601), 1200, 0, true), Path::StoppedOrUnknown);
        assert_eq!(classify(Some(601), 1200, 2, false), Path::StoppedOrUnknown);
        assert_eq!(classify(Some(600), 1200, 1, false), Path::StoppedOrUnknown);
        assert_eq!(classify(None, 1200, 1, false), Path::StoppedOrUnknown);
        assert_eq!(classify(None, 1200, 0, false), Path::NoGate);
    }
}

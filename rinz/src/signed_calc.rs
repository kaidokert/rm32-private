use core::ops::{Add, AddAssign, Mul, Neg, Sub};

// ---- Single-source "1" ----
pub trait CustomOne {
    fn one() -> Self;
}

impl<T> CustomOne for T
where
    T: From<i8>,
{
    #[inline]
    fn one() -> Self {
        1i8.into()
    }
}

// ---- Signed math bound for your filter ----
pub trait SignedCalc:
    Neg<Output = Self>
    + CustomOne
    + Default
    + Mul<Output = Self>
    + Sub<Output = Self>
    + Add<Output = Self>
    + AddAssign<Self>
    + PartialEq
    + PartialOrd
    + Copy
{
    fn zero() -> Self {
        Default::default()
    }
}
impl<T> SignedCalc for T where
    T: Neg<Output = Self>
        + CustomOne
        + Default
        + Mul<Output = Self>
        + Sub<Output = Self>
        + Add<Output = Self>
        + AddAssign<Self>
        + PartialEq
        + PartialOrd
        + Copy
{
}

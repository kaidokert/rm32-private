use core::ops::{Add, AddAssign, Mul, Neg, Sub};

c0nst::c0nst! {

// ---- Single-source "0" ----
pub c0nst trait CustomZero {
    fn zero() -> Self;
}

impl<T> CustomZero for T
where
    T: From<i8>,
{
    #[inline]
    fn zero() -> Self {
        0i8.into()
    }
}


// ---- Single-source "1" ----
pub c0nst trait CustomOne {
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

}

// ---- Signed math bound for your filter ----
pub trait SignedCalc:
    Neg<Output = Self>
    + CustomZero
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
impl<T> SignedCalc for T where
    T: Neg<Output = Self>
        + CustomZero
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

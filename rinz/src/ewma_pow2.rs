use crate::filter::FilterGeneric;
use crate::signed_calc::SignedCalc;
use core::marker::PhantomData;
use core::ops::Shr;

pub trait Shiftable: SignedCalc + Shr<usize, Output = Self> {}
impl<T> Shiftable for T where T: SignedCalc + Shr<usize, Output = T> {}

/// Multiply-free EWMA: y += (x − y) >> K  ⇒  α = 1 / 2^K
pub struct EwmaPow2<const K: usize, T: Shiftable> {
    _pd: PhantomData<T>,
}

impl<const K: usize, T: Shiftable> EwmaPow2<K, T> {
    pub const fn new() -> Self {
        Self { _pd: PhantomData }
    }
}

impl<const K: usize, T: Shiftable> FilterGeneric<T> for EwmaPow2<K, T> {
    type State = T;

    #[inline]
    fn filter(&self, x: T, y: &mut Self::State) -> T {
        let next = (*y) + ((x - *y) >> K);
        *y = next;
        next
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ewma_pow2() {
        let ewma = EwmaPow2::<1, i32>::new();
        let mut state: i32 = 0;
        let out = ewma.filter(100, &mut state);
        assert_eq!(out, 50);
        let out2 = ewma.filter(100, &mut state);
        assert_eq!(out2, 75);
    }
}

use crate::math::{From, SignedCast, UnsignedCast};
use num_traits::PrimInt;
use std::num::TryFromIntError;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Interval<T: PrimInt> {
    pub min: T,
    pub max: T,
}

impl<T: PrimInt> Interval<T> {
    pub fn new(min: T, max: T) -> Self {
        Self { min, max }
    }

    pub fn length(self) -> T {
        let len = self.max - self.min + T::one();
        std::cmp::max(len, T::zero())
    }

    pub fn contains<U: PrimInt>(self, value: U) -> bool {
        T::from(value).map_or(false, |n| self.min <= n && n <= self.max)
    }

    pub fn intersection(a: Self, b: Self) -> Self {
        let min = std::cmp::max(a.min, b.min);
        let max = std::cmp::min(a.max, b.max);
        Self::new(min, max)
    }

    pub fn is_empty_set(self) -> bool {
        self.min > self.max
    }
}

impl<U> Interval<U>
where
    U: PrimInt + SignedCast,
    <U as SignedCast>::Signed: PrimInt,
{
    pub fn to_signed(self) -> Result<Interval<U::Signed>, TryFromIntError> {
        let min = self.min.to_signed()?;
        let max = self.max.to_signed()?;
        Ok(Interval::new(min, max))
    }
}

impl<S> Interval<S>
where
    S: PrimInt + UnsignedCast,
    <S as UnsignedCast>::Unsigned: PrimInt,
{
    pub fn to_unsigned(self) -> Result<Interval<S::Unsigned>, TryFromIntError> {
        let min = self.min.to_unsigned()?;
        let max = self.max.to_unsigned()?;
        Ok(Interval::new(min, max))
    }
}

impl<T: PrimInt, U: PrimInt> From<Interval<U>> for Interval<T> {
    fn from(interval: &Interval<U>) -> Option<Self> {
        let min = T::from(interval.min)?;
        let max = T::from(interval.max)?;
        Some(Self::new(min, max))
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn intersection() {
        // Case 0: Intervals are the same
        let interval = Interval::new(-10, 10);
        assert_eq!(Interval::intersection(interval, interval), interval);

        // Case 1: One interval is contained in another
        let interval_a = Interval::new(-10, 10);
        let interval_b = Interval::new(0, 5);
        assert_eq!(Interval::intersection(interval_a, interval_b), interval_b);
        assert_eq!(Interval::intersection(interval_b, interval_a), interval_b);

        // Case 2: Intervals are partially overlapping
        let interval_a = Interval::new(-10, 10);
        let interval_b = Interval::new(5, 15);
        assert_eq!(Interval::intersection(interval_a, interval_b), Interval::new(5, 10));
        assert_eq!(Interval::intersection(interval_b, interval_a), Interval::new(5, 10));

        // Case 3: Intervals are disjoint
        let interval_a = Interval::new(-10, 10);
        let interval_b = Interval::new(11, 12);
        assert!(Interval::intersection(interval_a, interval_b).is_empty_set());
        assert!(Interval::intersection(interval_b, interval_a).is_empty_set());
    }
}

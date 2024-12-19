use crate::math::{From, SignedCast, UnsignedCast};
use num_traits::PrimInt;
use std::num::TryFromIntError;
use std::ops::Range;

/// Represents an inclusive interval.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Interval<T: PrimInt> {
    start: T,
    end: T,
}

impl<T: PrimInt> Interval<T> {
    pub fn new(min: T, max: T) -> Self {
        Self {
            start: min,
            end: max + T::one(),
        }
    }

    pub fn min(self) -> T {
        self.start
    }

    pub fn max(self) -> T {
        self.end - T::one()
    }

    pub fn length(self) -> T {
        let len = self.end - self.start;
        std::cmp::max(len, T::zero())
    }

    #[cfg(test)]
    pub fn is_empty_set(self) -> bool {
        self.start >= self.end
    }

    pub fn contains<U: PrimInt>(self, value: U) -> bool {
        T::from(value).map_or(false, |n| self.start <= n && n < self.end)
    }

    pub fn intersection(a: Self, b: Self) -> Self {
        let start = std::cmp::max(a.start, b.start);
        let end = std::cmp::min(a.end, b.end);
        Self { start, end }
    }

    pub fn iter(self) -> Range<T> {
        self.start..self.end
    }
}

impl<U> Interval<U>
where
    U: PrimInt + SignedCast,
    <U as SignedCast>::Signed: PrimInt,
{
    pub fn to_signed(self) -> Result<Interval<U::Signed>, TryFromIntError> {
        let start = self.start.to_signed()?;
        let end = self.end.to_signed()?;
        Ok(Interval { start, end })
    }
}

impl<S> Interval<S>
where
    S: PrimInt + UnsignedCast,
    <S as UnsignedCast>::Unsigned: PrimInt,
{
    pub fn to_unsigned(self) -> Result<Interval<S::Unsigned>, TryFromIntError> {
        let start = self.start.to_unsigned()?;
        let end = self.end.to_unsigned()?;
        Ok(Interval { start, end })
    }
}

impl<T: PrimInt, U: PrimInt> From<Interval<U>> for Interval<T> {
    fn from(interval: &Interval<U>) -> Option<Self> {
        let start = T::from(interval.start)?;
        let end = T::from(interval.end)?;
        Some(Interval { start, end })
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

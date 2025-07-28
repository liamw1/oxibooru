use crate::math::{From, SignedCast, UnsignedCast};
use num_traits::PrimInt;
use std::num::TryFromIntError;

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
        match self.is_empty_set() {
            true => T::zero(),
            false => self.end - self.start,
        }
    }

    pub fn contains<U: PrimInt>(self, value: U) -> bool {
        T::from(value).is_some_and(|n| self.start <= n && n < self.end)
    }

    pub fn is_empty_set(self) -> bool {
        self.start >= self.end
    }

    pub fn intersection(a: Self, b: Self) -> Self {
        let start = std::cmp::max(a.start, b.start);
        let end = std::cmp::min(a.end, b.end);
        Self { start, end }
    }

    pub fn linspace<const N: usize>(self) -> [T; N] {
        let min_f64 = self.min().to_f64().unwrap();
        let max_f64 = self.max().to_f64().unwrap();
        let num_f64 = f64::from(u32::try_from(N).unwrap());

        let mut arr = [T::zero(); N];
        match N {
            0 => (),
            1 => {
                let midpoint = 0.5 * min_f64 + 0.5 * max_f64;
                arr[0] = T::from(midpoint).unwrap();
            }
            _ => {
                let step = (max_f64 - min_f64) / (num_f64 - 1.0);
                for (i, item) in arr.iter_mut().enumerate() {
                    let index = f64::from(u32::try_from(i).unwrap());
                    let point = (min_f64 + index * step).round();
                    *item = T::from(point).unwrap();
                }
            }
        }
        arr
    }

    pub fn shrink(&mut self, n: T) {
        self.start = self.start.saturating_add(n);
        self.end = self.end.saturating_sub(n);
        if self.is_empty_set() {
            let midpoint = self.end + (self.start - self.end) / (T::one() + T::one());
            self.start = midpoint;
            self.end = midpoint + T::one();
        }
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

    #[test]
    fn create_linspace() {
        assert_eq!(Interval::new(0, 2).linspace(), [0; 0]);
        assert_eq!(Interval::new(0, 2).linspace(), [1]);
        assert_eq!(Interval::new(0, 2).linspace(), [0, 2]);
        assert_eq!(Interval::new(0, 2).linspace(), [0, 1, 2]);
        assert_eq!(Interval::new(0, 5).linspace(), [0, 3, 5]);
        assert_eq!(Interval::new(0, 100).linspace(), [0, 20, 40, 60, 80, 100]);
        assert_eq!(Interval::new(0, 100).linspace(), [0, 17, 33, 50, 67, 83, 100]);
        assert_eq!(Interval::new(100, 0).linspace(), [100, 83, 67, 50, 33, 17, 0]);
    }
}

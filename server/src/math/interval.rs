use num_traits::PrimInt;

/// Represents an integer interval.
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

    /// Calculates the midpoint of the interval, avoiding overflow
    pub fn midpoint(self) -> T {
        self.start + (self.end - self.start) / (T::one() + T::one())
    }

    pub fn length(self) -> T {
        if self.is_empty_set() {
            T::zero()
        } else {
            self.end - self.start
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
        match N {
            0 => [T::zero(); N],
            1 => [self.midpoint(); N],
            _ => {
                const CAST_MESSAGE: &str = "Interval should be contained within range of valid f64 values";
                let min_f64 = self.min().to_f64().expect(CAST_MESSAGE);
                let max_f64 = self.max().to_f64().expect(CAST_MESSAGE);

                let num_u32 = u32::try_from(N).expect("N should not exceed u32::MAX");
                let step = (max_f64 - min_f64) / (f64::from(num_u32) - 1.0);

                let mut arr = [T::zero(); N];
                for (item, index) in arr.iter_mut().zip(0..num_u32) {
                    let point = (min_f64 + f64::from(index) * step).round();
                    *item = T::from(point).expect("Linspace point must be within interval bounds");
                }
                arr
            }
        }
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

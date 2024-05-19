use num_traits::PrimInt;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Interval<I: PrimInt> {
    pub min: I,
    pub max: I,
}

impl<I: PrimInt> Interval<I> {
    pub fn new(min: I, max: I) -> Self {
        Self { min, max }
    }

    pub fn length(&self) -> I {
        let len = self.max - self.min + I::one();
        std::cmp::max(len, I::zero())
    }

    pub fn contains(&self, n: I) -> bool {
        self.min <= n && n <= self.max
    }

    pub fn intersection(a: Self, b: Self) -> Self {
        let min = std::cmp::max(a.min, b.min);
        let max = std::cmp::min(a.max, b.max);
        Self { min, max }
    }

    pub fn is_empty_set(&self) -> bool {
        self.min > self.max
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

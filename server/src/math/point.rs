use num_traits::int::PrimInt;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct IPoint2<I: PrimInt> {
    pub i: I,
    pub j: I,
}

impl<I: PrimInt> IPoint2<I> {
    pub fn new(i: I, j: I) -> Self {
        Self { i, j }
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn lexicographical_ordering() {
        assert!(IPoint2::new(1, 0) > IPoint2::new(0, 0));
        assert!(IPoint2::new(0, 1) > IPoint2::new(0, 0));
        assert!(IPoint2::new(1, 0) > IPoint2::new(0, 1));
        assert!(IPoint2::new(0, 8) < IPoint2::new(1, 0));

        assert!(IPoint2::new(1, 1) > IPoint2::new(1, 0));
        assert!(IPoint2::new(1, 1) < IPoint2::new(1, 2));
        assert!(IPoint2::new(3, 0) > IPoint2::new(2, 2));
    }
}

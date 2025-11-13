use num_traits::int::PrimInt;
use std::convert::TryFrom;
use std::num::TryFromIntError;

/// Represents a point on a two-dimensional integer lattice.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct IPoint2<T: PrimInt> {
    pub i: T,
    pub j: T,
}

impl<T: PrimInt> IPoint2<T> {
    /// Constructs a point at coordinates (`i`, `j`).
    pub fn new(i: T, j: T) -> Self {
        Self { i, j }
    }
}

impl TryFrom<IPoint2<i64>> for IPoint2<u32> {
    type Error = TryFromIntError;
    fn try_from(value: IPoint2<i64>) -> Result<Self, Self::Error> {
        Ok(Self {
            j: u32::try_from(value.j)?,
            i: u32::try_from(value.i)?,
        })
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

use crate::math::{From, SignedCast};
use num_traits::int::PrimInt;
use std::num::TryFromIntError;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct IPoint2<T: PrimInt> {
    pub i: T,
    pub j: T,
}

impl<T: PrimInt> IPoint2<T> {
    pub fn new(i: T, j: T) -> Self {
        Self { i, j }
    }

    #[cfg(test)]
    pub fn zero() -> Self {
        Self::new(T::zero(), T::zero())
    }
}

impl<U> IPoint2<U>
where
    U: PrimInt + SignedCast,
    <U as SignedCast>::Signed: PrimInt,
{
    pub fn to_signed(self) -> Result<IPoint2<U::Signed>, TryFromIntError> {
        let i = self.i.to_signed()?;
        let j = self.j.to_signed()?;
        Ok(IPoint2::new(i, j))
    }
}

impl<T: PrimInt, U: PrimInt> From<IPoint2<U>> for IPoint2<T> {
    fn from(point: &IPoint2<U>) -> Option<Self> {
        let i = T::from(point.i)?;
        let j = T::from(point.j)?;
        Some(Self::new(i, j))
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

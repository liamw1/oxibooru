use crate::math::interval::Interval;
use crate::math::point::IPoint2;
use crate::math::From;
use crate::math::{SignedCast, UnsignedCast};
use num_traits::int::PrimInt;
use std::num::TryFromIntError;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct IRect<T: PrimInt> {
    i_bounds: Interval<T>,
    j_bounds: Interval<T>,
}

impl<T: PrimInt> IRect<T> {
    pub fn new(i_bounds: Interval<T>, j_bounds: Interval<T>) -> Self {
        Self { i_bounds, j_bounds }
    }

    pub fn new_zero_based(i_max: T, j_max: T) -> Self {
        let i_bounds = Interval::new(T::zero(), i_max);
        let j_bounds = Interval::new(T::zero(), j_max);
        Self::new(i_bounds, j_bounds)
    }

    pub fn new_zero_based_square(max: T) -> Self {
        let bounds = Interval::new(T::zero(), max);
        Self::new(bounds, bounds)
    }

    pub fn new_centered_square(center: IPoint2<T>, radius: T) -> Self {
        let i_bounds = Interval::new(center.i - radius, center.i + radius);
        let j_bounds = Interval::new(center.j - radius, center.j + radius);
        Self::new(i_bounds, j_bounds)
    }

    pub fn contains<U: PrimInt>(&self, point: IPoint2<U>) -> bool {
        self.i_bounds.contains(point.i) && self.j_bounds.contains(point.j)
    }

    pub fn intersection(a: Self, b: Self) -> Self {
        let i_bounds = Interval::intersection(a.i_bounds, b.i_bounds);
        let j_bounds = Interval::intersection(a.j_bounds, b.j_bounds);
        Self::new(i_bounds, j_bounds)
    }

    pub fn is_empty_set(&self) -> bool {
        self.i_bounds.is_empty_set() || self.j_bounds.is_empty_set()
    }

    pub fn extents(&self) -> IPoint2<T> {
        IPoint2::new(self.i_bounds.length(), self.j_bounds.length())
    }

    pub fn total_points(&self) -> Option<u64> {
        self.i_bounds
            .length()
            .checked_mul(&self.j_bounds.length())
            .and_then(|prod| prod.to_u64())
    }

    pub fn min_corner(&self) -> IPoint2<T> {
        IPoint2::new(self.i_bounds.min(), self.j_bounds.min())
    }

    pub fn max_corner(&self) -> IPoint2<T> {
        IPoint2::new(self.i_bounds.max(), self.j_bounds.max())
    }

    pub fn iter(&self) -> IRectIter<T> {
        IRectIter {
            rect: *self,
            current: self.min_corner(),
        }
    }
}

impl<U> IRect<U>
where
    U: PrimInt + SignedCast,
    <U as SignedCast>::Signed: PrimInt,
{
    pub fn to_signed(&self) -> Result<IRect<U::Signed>, TryFromIntError> {
        let i_bounds = self.i_bounds.to_signed()?;
        let j_bounds = self.j_bounds.to_signed()?;
        Ok(IRect::new(i_bounds, j_bounds))
    }
}

impl<S> IRect<S>
where
    S: PrimInt + UnsignedCast,
    <S as UnsignedCast>::Unsigned: PrimInt,
{
    pub fn to_unsigned(&self) -> Result<IRect<S::Unsigned>, TryFromIntError> {
        let i_bounds = self.i_bounds.to_unsigned()?;
        let j_bounds = self.j_bounds.to_unsigned()?;
        Ok(IRect::new(i_bounds, j_bounds))
    }
}

impl<T: PrimInt, U: PrimInt> From<IRect<U>> for IRect<T> {
    fn from(rect: &IRect<U>) -> Option<Self> {
        let i_bounds = <Interval<T> as From<Interval<U>>>::from(&rect.i_bounds)?;
        let j_bounds = <Interval<T> as From<Interval<U>>>::from(&rect.j_bounds)?;
        Some(Self::new(i_bounds, j_bounds))
    }
}

pub struct IRectIter<T: PrimInt> {
    rect: IRect<T>,
    current: IPoint2<T>,
}

impl<T> Iterator for IRectIter<T>
where
    T: PrimInt + std::ops::AddAssign<T>,
{
    type Item = IPoint2<T>;
    fn next(&mut self) -> Option<Self::Item> {
        if self.current > self.rect.max_corner() {
            return None;
        }

        let result = self.current;

        if self.current.j < self.rect.max_corner().j {
            self.current.j += T::one();
        } else {
            self.current.j = self.rect.min_corner().j;
            self.current.i += T::one();
        }

        Some(result)
    }
}

pub struct Array2D<T> {
    data: Vec<T>,
    dimensions: (u32, u32),
}

impl<T> Array2D<T>
where
    T: Copy,
{
    pub fn new(dimensions: (u32, u32), initial_value: T) -> Self {
        let array_size = dimensions.0 * dimensions.1;
        Array2D {
            data: vec![initial_value; array_size as usize],
            dimensions,
        }
    }

    pub fn new_square(dimensions: u32, initial_value: T) -> Self {
        Self::new((dimensions, dimensions), initial_value)
    }

    pub fn bounds(&self) -> IRect<u32> {
        IRect::new_zero_based(self.dimensions.0 - 1, self.dimensions.1 - 1)
    }

    pub fn at(&self, index: IPoint2<u32>) -> T {
        self.data[self.compute_linear_index(index)]
    }

    pub fn set_at<I: PrimInt>(&mut self, index: IPoint2<I>, value: T) {
        let converted_index = <IPoint2<u32> as From<IPoint2<I>>>::from(&index).unwrap();
        let index = self.compute_linear_index(converted_index);
        self.data[index] = value;
    }

    pub fn get<I: PrimInt>(&self, index: IPoint2<I>) -> Option<T> {
        let converted_index = <IPoint2<u32> as From<IPoint2<I>>>::from(&index)?;
        match self.bounds().contains(index) {
            false => None,
            true => Some(self.at(converted_index)),
        }
    }

    pub fn iter(&self) -> std::slice::Iter<T> {
        self.data.iter()
    }

    pub fn enumerate(&self) -> std::iter::Zip<IRectIter<u32>, std::slice::Iter<T>> {
        self.bounds().iter().zip(self.iter())
    }

    pub fn signed_enumerate(&self) -> std::iter::Zip<IRectIter<i32>, std::slice::Iter<T>> {
        let signed_bounds = self.bounds().to_signed().unwrap();
        signed_bounds.iter().zip(self.iter())
    }

    pub fn iter_mut(&mut self) -> std::slice::IterMut<T> {
        self.data.iter_mut()
    }

    pub fn enumerate_mut(&mut self) -> std::iter::Zip<IRectIter<u32>, std::slice::IterMut<T>> {
        self.bounds().iter().zip(self.iter_mut())
    }

    fn compute_linear_index(&self, index: IPoint2<u32>) -> usize {
        self.dimensions.1 as usize * index.i as usize + index.j as usize
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn rect_iteration() {
        let check_square_rect = |min_corner: IPoint2<i32>| {
            let center = IPoint2::new(min_corner.i + 1, min_corner.j + 1);
            let rect = IRect::new_centered_square(center, 1);
            let values = rect
                .iter()
                .map(|point| 10 * (point.i - min_corner.i) + (point.j - min_corner.j))
                .collect::<Vec<_>>();
            assert_eq!(values, vec![00, 01, 02, 10, 11, 12, 20, 21, 22]);
        };
        let check_non_square_rect = |min_corner: IPoint2<i32>| {
            let i_bounds = Interval::new(min_corner.i, min_corner.i + 2);
            let j_bounds = Interval::new(min_corner.j, min_corner.j + 3);
            let rect = IRect::new(i_bounds, j_bounds);
            let values = rect
                .iter()
                .map(|point| 10 * (point.i - min_corner.i) + (point.j - min_corner.j))
                .collect::<Vec<_>>();
            assert_eq!(values, vec![00, 01, 02, 03, 10, 11, 12, 13, 20, 21, 22, 23])
        };

        for i in -3..3 {
            for j in -3..3 {
                let min_corner = IPoint2::new(i, j);
                check_square_rect(min_corner);
                check_non_square_rect(min_corner);
            }
        }
    }

    #[test]
    fn intersection() {
        let origin = IPoint2::zero();

        // Case 0: Regions are the same
        let region = IRect::new_centered_square(origin, 10);
        assert_eq!(IRect::intersection(region, region), region);

        // Case 1: One region is contained in another
        let region_a = IRect::new_centered_square(origin, 10);
        let region_b = IRect::new_centered_square(origin, 5);
        assert_eq!(IRect::intersection(region_a, region_b), region_b);
        assert_eq!(IRect::intersection(region_b, region_a), region_b);

        // Case 2: Regions are partially overlapping
        let region_a = IRect::new_centered_square(origin, 10);
        let region_b = IRect::new_centered_square(IPoint2::new(5, 5), 10);
        let expected_result = IRect::new(Interval::new(-5, 10), Interval::new(-5, 10));
        assert_eq!(IRect::intersection(region_a, region_b), expected_result);
        assert_eq!(IRect::intersection(region_b, region_a), expected_result);

        // Case 3: Regions are disjoint
        let region_a = IRect::new_centered_square(origin, 10);
        let region_b = IRect::new_centered_square(IPoint2::new(12, 12), 1);
        assert!(IRect::intersection(region_a, region_b).is_empty_set());
        assert!(IRect::intersection(region_b, region_a).is_empty_set());
    }

    #[test]
    fn array_2d() {
        let array_rect = Array2D::new_square(3, 5);
        assert_eq!(array_rect.at(IPoint2::zero()), 5);
        assert_eq!(array_rect.at(IPoint2::new(2, 2)), 5);
    }
}

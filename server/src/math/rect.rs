use crate::math::interval::Interval;
use crate::math::point::IPoint2;
use num_traits::int::PrimInt;
use std::fmt::Debug;
use std::ops::AddAssign;

/// Represents a box on a two-dimensional integer lattice.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct IRect<T: PrimInt> {
    i_bounds: Interval<T>,
    j_bounds: Interval<T>,
}

impl<T: PrimInt> IRect<T> {
    /// Constructs a new [`IRect`] from two intervals `i_bounds` and `j_bounds`.
    pub fn new(i_bounds: Interval<T>, j_bounds: Interval<T>) -> Self {
        Self { i_bounds, j_bounds }
    }

    /// Constructs a new [`IRect`] with a min-corner at the origin and a max-corner at \[`i_max`, `j_max`\].
    pub fn new_zero_based(i_max: T, j_max: T) -> Self {
        let i_bounds = Interval::new(T::zero(), i_max);
        let j_bounds = Interval::new(T::zero(), j_max);
        Self::new(i_bounds, j_bounds)
    }

    /// Constructs a new square [`IRect`] centered on `center` and which extends `radius` units in the cardinal directions.
    pub fn new_centered_square(center: IPoint2<T>, radius: T) -> Self {
        let i_bounds = Interval::new(center.i - radius, center.i + radius);
        let j_bounds = Interval::new(center.j - radius, center.j + radius);
        Self::new(i_bounds, j_bounds)
    }

    /// Determines if given `point` is contained within the [`IRect`].
    pub fn contains<U: PrimInt>(&self, point: IPoint2<U>) -> bool {
        self.i_bounds.contains(point.i) && self.j_bounds.contains(point.j)
    }

    /// Computes the intersection of two [`IRect`]s `a` and `b`.
    pub fn intersection(a: Self, b: Self) -> Self {
        let i_bounds = Interval::intersection(a.i_bounds, b.i_bounds);
        let j_bounds = Interval::intersection(a.j_bounds, b.j_bounds);
        Self::new(i_bounds, j_bounds)
    }

    /// Computes the total number of points the [`IRect`] contains. Returns [`None`] on overflow.
    pub fn total_points(&self) -> Option<u64> {
        let width = self.i_bounds.length().to_u64()?;
        let height = self.j_bounds.length().to_u64()?;
        width.checked_mul(height)
    }

    /// Returns the lower left corner of the [`IRect`].
    pub fn min_corner(&self) -> IPoint2<T> {
        IPoint2::new(self.i_bounds.min(), self.j_bounds.min())
    }

    /// Returns the upper right corner of the [`IRect`].
    pub fn max_corner(&self) -> IPoint2<T> {
        IPoint2::new(self.i_bounds.max(), self.j_bounds.max())
    }

    /// Returns an iterator over the points contained within the [`IRect`].
    pub fn iter(&self) -> IRectIter<T> {
        IRectIter {
            rect: *self,
            current: self.min_corner(),
        }
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

        let next = self.current;

        if self.current.j < self.rect.max_corner().j {
            self.current.j += T::one();
        } else {
            self.current.j = self.rect.min_corner().j;
            self.current.i += T::one();
        }

        Some(next)
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let size = self.rect.total_points().and_then(|size| usize::try_from(size).ok());
        (size.unwrap_or(usize::MAX), size)
    }
}

/// Represents data of type `T` stored in a two-dimensional array with dimensions `N` and `M`.
pub struct Array2D<T, const N: usize, const M: usize> {
    data: [[T; M]; N],
}

impl<T, const N: usize, const M: usize> Array2D<T, N, M>
where
    T: Copy,
{
    /// Constructs a new [`Array2D`] where every value is initialized to given `initial_value`.
    pub fn new(initial_value: T) -> Self {
        Array2D {
            data: [[initial_value; M]; N],
        }
    }

    /// Returns the bounds of the array as an [`IRect`].
    pub fn bounds<I>() -> IRect<I>
    where
        I: PrimInt + TryFrom<usize>,
        <I as TryFrom<usize>>::Error: Debug,
    {
        IRect::new_zero_based(
            I::try_from(N.saturating_sub(1)).unwrap_or(I::min_value()),
            I::try_from(N.saturating_sub(1)).unwrap_or(I::max_value()),
        )
    }

    /// Retrieves data stored at given two-dimensional `index`. Panics if index is out-of-bounds.
    pub fn at<I: PrimInt>(&self, index: IPoint2<I>) -> T {
        self.data[index.i.to_usize().unwrap()][index.j.to_usize().unwrap()]
    }

    /// Sets data stored at given two-dimensional `index` to `value`. Panics if index is out-of-bounds.
    pub fn set_at<I: PrimInt>(&mut self, index: IPoint2<I>, value: T) {
        self.data[index.i.to_usize().unwrap()][index.j.to_usize().unwrap()] = value;
    }

    /// Retrieves data stores at given two-dimensional `index`. Returns [`None`] if index is out-of-bounds.
    pub fn get<I>(&self, index: IPoint2<I>) -> Option<T>
    where
        I: PrimInt + TryFrom<usize>,
        <I as TryFrom<usize>>::Error: Debug,
    {
        Self::bounds::<I>().contains(index).then(|| self.at(index))
    }

    /// Returns an iterator over the stored elements.
    pub fn iter(&self) -> impl Iterator<Item = &T> {
        self.data.iter().flatten()
    }

    /// Returns an iterator over the stored elements and their corresponding indices within the array.
    pub fn indexed_iter<I>(&self) -> impl Iterator<Item = (IPoint2<I>, &T)>
    where
        I: PrimInt + TryFrom<usize> + AddAssign,
        <I as TryFrom<usize>>::Error: Debug,
    {
        Self::bounds::<I>().iter().zip(self.iter())
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
            assert_eq!(values, vec![0, 1, 2, 10, 11, 12, 20, 21, 22]);
        };
        let check_non_square_rect = |min_corner: IPoint2<i32>| {
            let i_bounds = Interval::new(min_corner.i, min_corner.i + 2);
            let j_bounds = Interval::new(min_corner.j, min_corner.j + 3);
            let rect = IRect::new(i_bounds, j_bounds);
            let values = rect
                .iter()
                .map(|point| 10 * (point.i - min_corner.i) + (point.j - min_corner.j))
                .collect::<Vec<_>>();
            assert_eq!(values, vec![0, 1, 2, 3, 10, 11, 12, 13, 20, 21, 22, 23]);
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
        let origin = IPoint2::new(0, 0);

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
        assert_eq!(IRect::intersection(region_a, region_b).total_points(), Some(0));
        assert_eq!(IRect::intersection(region_b, region_a).total_points(), Some(0));
    }

    #[test]
    fn array_2d() {
        let array_rect: Array2D<_, 3, 3> = Array2D::new(5);
        assert_eq!(array_rect.at(IPoint2::<i32>::new(0, 0)), 5);
        assert_eq!(array_rect.at(IPoint2::new(2, 2)), 5);
    }
}

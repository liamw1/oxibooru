use crate::math::interval::Interval;
use crate::math::point::IPoint2;
use num_traits::int::PrimInt;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct IRect<I: PrimInt> {
    i_bounds: Interval<I>,
    j_bounds: Interval<I>,
}

impl<I: PrimInt> IRect<I> {
    pub fn new(i_bounds: Interval<I>, j_bounds: Interval<I>) -> Self {
        Self { i_bounds, j_bounds }
    }

    pub fn new_zero_based(i_max: I, j_max: I) -> Self {
        let i_bounds = Interval::new(I::zero(), i_max);
        let j_bounds = Interval::new(I::zero(), j_max);
        Self::new(i_bounds, j_bounds)
    }

    pub fn new_zero_based_square(max: I) -> Self {
        let bounds = Interval::new(I::zero(), max);
        Self::new(bounds, bounds)
    }

    pub fn new_centered_square(center: IPoint2<I>, radius: I) -> Self {
        let i_bounds = Interval::new(center.i - radius, center.i + radius);
        let j_bounds = Interval::new(center.j - radius, center.j + radius);
        Self::new(i_bounds, j_bounds)
    }

    pub fn contains(&self, point: IPoint2<I>) -> bool {
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

    pub fn extents(&self) -> IPoint2<I> {
        IPoint2::new(self.i_bounds.length(), self.j_bounds.length())
    }

    pub fn total_points(&self) -> Option<usize> {
        self.i_bounds
            .length()
            .checked_mul(&self.j_bounds.length())
            .and_then(|prod| prod.to_usize())
    }

    pub fn min_corner(&self) -> IPoint2<I> {
        IPoint2::new(self.i_bounds.min, self.j_bounds.min)
    }

    pub fn max_corner(&self) -> IPoint2<I> {
        IPoint2::new(self.i_bounds.max, self.j_bounds.max)
    }

    pub fn iter(&self) -> IRectIter<I> {
        IRectIter {
            rect: *self,
            current: self.min_corner(),
        }
    }
}

pub struct IRectIter<I: PrimInt> {
    rect: IRect<I>,
    current: IPoint2<I>,
}

impl<I> Iterator for IRectIter<I>
where
    I: PrimInt + std::ops::AddAssign<I>,
{
    type Item = IPoint2<I>;
    fn next(&mut self) -> Option<Self::Item> {
        if self.current > self.rect.max_corner() {
            return None;
        }

        let result = self.current;

        if self.current.j < self.rect.max_corner().j {
            self.current.j += I::one();
        } else {
            self.current.j = self.rect.min_corner().j;
            self.current.i += I::one();
        }

        Some(result)
    }
}

pub struct Array2D<T> {
    data: Vec<T>,
    dimensions: (usize, usize),
}

impl<T> Array2D<T>
where
    T: Copy,
{
    pub fn new(dimensions: (usize, usize), initial_value: T) -> Option<Self> {
        dimensions.0.checked_mul(dimensions.1).map(|array_size| Array2D {
            data: vec![initial_value; array_size],
            dimensions,
        })
    }

    pub fn new_square(dimensions: usize, initial_value: T) -> Option<Self> {
        Self::new((dimensions, dimensions), initial_value)
    }

    pub fn bounds(&self) -> IRect<usize> {
        IRect::new_zero_based(self.dimensions.0, self.dimensions.1)
    }

    pub fn at(&self, index: IPoint2<usize>) -> T {
        self.data[self.compute_linear_index(index)]
    }

    pub fn set_at(&mut self, index: IPoint2<usize>, value: T) {
        let index = self.compute_linear_index(index);
        self.data[index] = value;
    }

    pub fn index_iter(&self) -> IRectIter<usize> {
        self.bounds().iter()
    }

    pub fn iter(&self) -> std::slice::Iter<'_, T> {
        self.data.iter()
    }

    fn compute_linear_index(&self, index: IPoint2<usize>) -> usize {
        self.dimensions.1 * index.i + index.j
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
        let array_rect = Array2D::new_square(3, 5).unwrap();
        assert_eq!(array_rect.at(IPoint2::zero()), 5);
        assert_eq!(array_rect.at(IPoint2::new(2, 2)), 5);
    }
}

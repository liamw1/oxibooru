use crate::math::interval::Interval;
use crate::math::point::IPoint2;
use num_traits::int::PrimInt;

#[derive(Clone, Copy)]
pub struct IRect<I: PrimInt> {
    i_bounds: Interval<I>,
    j_bounds: Interval<I>,
}

impl<I: PrimInt> IRect<I> {
    pub fn new(i_bounds: Interval<I>, j_bounds: Interval<I>) -> Self {
        Self { i_bounds, j_bounds }
    }

    pub fn new_square(max: I) -> Self {
        let bounds = Interval::new(I::zero(), max);
        Self {
            i_bounds: bounds,
            j_bounds: bounds,
        }
    }

    pub fn new_centered_square(center: IPoint2<I>, radius: I) -> Self {
        let i_bounds = Interval::new(center.i - radius, center.i + radius);
        let j_bounds = Interval::new(center.j - radius, center.j + radius);
        Self { i_bounds, j_bounds }
    }

    pub fn extents(&self) -> IPoint2<I> {
        IPoint2::new(self.i_bounds.length(), self.j_bounds.length())
    }

    pub fn total_points(&self) -> Option<usize> {
        (self.i_bounds.length() * self.i_bounds.length()).to_usize()
    }

    pub fn min_corner(&self) -> IPoint2<I> {
        IPoint2::new(self.i_bounds.min, self.j_bounds.min)
    }

    pub fn max_corner(&self) -> IPoint2<I> {
        IPoint2::new(self.i_bounds.max, self.j_bounds.max)
    }

    pub fn iter<'a>(&'a self) -> IRectIter<'a, I> {
        IRectIter {
            rect: self,
            current: self.min_corner(),
        }
    }
}

pub struct IRectIter<'a, I: PrimInt> {
    rect: &'a IRect<I>,
    current: IPoint2<I>,
}

impl<'a, I> Iterator for IRectIter<'a, I>
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

pub struct ArrayRect<T, I: PrimInt> {
    data: Vec<T>,
    bounds: IRect<I>,
}

impl<T, I> ArrayRect<T, I>
where
    T: Copy,
    I: PrimInt,
{
    pub fn new(bounds: IRect<I>, initial_value: T) -> Option<ArrayRect<T, I>> {
        bounds.total_points().map(|array_size| ArrayRect {
            data: vec![initial_value; array_size],
            bounds,
        })
    }

    pub fn at(&self, i: I, j: I) -> T {
        self.data[self.compute_linear_index(i, j)]
    }

    pub fn at_mut(&mut self, i: I, j: I) -> &mut T {
        let index = self.compute_linear_index(i, j);
        &mut self.data[index]
    }

    fn compute_linear_index(&self, i: I, j: I) -> usize {
        let stride = self.bounds.extents().j;
        let min_corner = self.bounds.min_corner();
        let index = stride * (i - min_corner.i) + (j - min_corner.j);
        index.to_usize().unwrap()
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
            let values: Vec<_> = rect
                .iter()
                .map(|point| 10 * (point.i - min_corner.i) + (point.j - min_corner.j))
                .collect();
            assert_eq!(values, vec![00, 01, 02, 10, 11, 12, 20, 21, 22]);
        };
        let check_non_square_rect = |min_corner: IPoint2<i32>| {
            let i_bounds = Interval::new(min_corner.i, min_corner.i + 2);
            let j_bounds = Interval::new(min_corner.j, min_corner.j + 3);
            let rect = IRect { i_bounds, j_bounds };
            let values: Vec<_> = rect
                .iter()
                .map(|point| 10 * (point.i - min_corner.i) + (point.j - min_corner.j))
                .collect();
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
    fn array_box() {
        let rect = IRect::new_square(2);
        let array_rect = ArrayRect::new(rect, 5).unwrap();

        assert_eq!(array_rect.at(0, 0), 5);
        assert_eq!(array_rect.at(2, 2), 5);
    }
}

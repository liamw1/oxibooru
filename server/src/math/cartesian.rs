use crate::math::point::IPoint2;
use crate::math::rect::IRect;

/// Represents the Cartesian product of two sets.
/// Provides methods for iterating over the product.
/// Space complexity is O(`N`+`M`).
pub struct CartesianProduct<L, R, const N: usize, const M: usize> {
    left: [L; N],
    right: [R; M],
}

impl<L, R, const N: usize, const M: usize> CartesianProduct<L, R, N, M> {
    pub fn new(left: [L; N], right: [R; M]) -> CartesianProduct<L, R, N, M> {
        CartesianProduct { left, right }
    }

    pub fn at(&self, i: usize, j: usize) -> (&L, &R) {
        (&self.left[i], &self.right[j])
    }

    #[cfg(test)]
    pub fn left_set(&self) -> &[L] {
        &self.left
    }

    #[cfg(test)]
    pub fn right_set(&self) -> &[R] {
        &self.right
    }

    pub fn bounds(&self) -> IRect<usize> {
        IRect::new_zero_based(self.left.len() - 1, self.right.len() - 1)
    }

    pub fn iter(&'_ self) -> CartesianProductIter<'_, L, R, N, M> {
        CartesianProductIter {
            cartesian_product: self,
            current: IPoint2::new(0, 0),
        }
    }

    pub fn indexed_iter(&self) -> impl Iterator<Item = (IPoint2<usize>, (&L, &R))> {
        // This is a bit wasteful, since CartesianProductIter already contains an index
        self.bounds().iter().zip(self.iter())
    }
}

pub struct CartesianProductIter<'a, L, R, const N: usize, const M: usize> {
    cartesian_product: &'a CartesianProduct<L, R, N, M>,
    current: IPoint2<usize>,
}

impl<'a, L, R, const N: usize, const M: usize> Iterator for CartesianProductIter<'a, L, R, N, M> {
    type Item = (&'a L, &'a R);
    fn next(&mut self) -> Option<Self::Item> {
        if self.current > self.cartesian_product.bounds().max_corner() {
            return None;
        }

        let current = self.current;

        if self.current.j < self.cartesian_product.right.len() - 1 {
            self.current.j += 1;
        } else {
            self.current.j = 0;
            self.current.i += 1;
        }

        Some(self.cartesian_product.at(current.i, current.j))
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let size = self
            .cartesian_product
            .left
            .len()
            .checked_mul(self.cartesian_product.right.len());
        (size.unwrap_or(usize::MAX), size)
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn cartesian_product_iteration() {
        // Sum of all possible products of two arithmetic progressions
        let expected_sum = |n: i32, m: i32| n * (n + 1) * m * (m + 1) / 4;

        let product = CartesianProduct::new([0, 1, 2, 3, 4], [0, 1, 2, 3, 4]);
        let sum = product.iter().map(|(l, r)| l * r).sum::<i32>();
        assert_eq!(sum, expected_sum(4, 4));

        let product = CartesianProduct::new([0, 1, 2, 3, 4, 5], [0, 1, 2, 3, 4]);
        let sum = product.iter().map(|(l, r)| l * r).sum::<i32>();
        assert_eq!(sum, expected_sum(5, 4));

        let product = CartesianProduct::new([0, 1, 2], [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10]);
        let sum = product.iter().map(|(l, r)| l * r).sum::<i32>();
        assert_eq!(sum, expected_sum(2, 10));
    }
}

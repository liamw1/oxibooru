use crate::math::point::IPoint2;
use crate::math::rect::IRect;

pub struct CartesianProduct<L, R> {
    left: Vec<L>,
    right: Vec<R>,
}

impl<L, R> CartesianProduct<L, R> {
    pub fn new(left: Vec<L>, right: Vec<R>) -> CartesianProduct<L, R> {
        CartesianProduct { left, right }
    }

    pub fn at(&self, i: usize, j: usize) -> (&L, &R) {
        (&self.left[i], &self.right[j])
    }

    pub fn left_set(&self) -> &[L] {
        &self.left
    }

    pub fn right_set(&self) -> &[R] {
        &self.right
    }

    pub fn bounds(&self) -> IRect<usize> {
        IRect::new_zero_based(self.left.len() - 1, self.right.len() - 1)
    }

    pub fn iter<'a>(&'a self) -> CartesianProductIter<'a, L, R> {
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

pub struct CartesianProductIter<'a, L, R> {
    cartesian_product: &'a CartesianProduct<L, R>,
    current: IPoint2<usize>,
}

impl<'a, L, R> Iterator for CartesianProductIter<'a, L, R> {
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

        let product = CartesianProduct::new(vec![0, 1, 2, 3, 4], vec![0, 1, 2, 3, 4]);
        let sum = product.iter().map(|(l, r)| l * r).sum::<i32>();
        assert_eq!(sum, expected_sum(4, 4));

        let product = CartesianProduct::new(vec![0, 1, 2, 3, 4, 5], vec![0, 1, 2, 3, 4]);
        let sum = product.iter().map(|(l, r)| l * r).sum::<i32>();
        assert_eq!(sum, expected_sum(5, 4));

        let product = CartesianProduct::new(vec![0, 1, 2], vec![0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10]);
        let sum = product.iter().map(|(l, r)| l * r).sum::<i32>();
        assert_eq!(sum, expected_sum(2, 10));
    }
}

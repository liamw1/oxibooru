use num_traits::Num;

#[derive(Clone, Copy)]
pub struct Interval<I: Num> {
    pub min: I,
    pub max: I,
}

impl<I: Num + Copy> Interval<I> {
    pub fn new(min: I, max: I) -> Interval<I> {
        Self { min, max }
    }

    pub fn length(&self) -> I {
        self.max - self.min + I::one()
    }
}

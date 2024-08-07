use num_traits::PrimInt;

pub fn linspace<T: PrimInt>(start: T, end: T, num: u32) -> Vec<T> {
    let start_f64 = start.to_f64().unwrap();
    let end_f64 = end.to_f64().unwrap();
    let num_f64 = num as f64;

    match num {
        0 => vec![],
        1 => {
            let midpoint = 0.5 * start_f64 + 0.5 * end_f64;
            vec![T::from(midpoint).unwrap()]
        }
        n => {
            let step = (end_f64 - start_f64) / (num_f64 - 1.0);
            (0..n)
                .map(|i| (start_f64 + i as f64 * step).round())
                .map(|x| T::from(x).unwrap())
                .collect()
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn create_linspace() {
        assert_eq!(linspace(0, 2, 0), Vec::<i32>::new());
        assert_eq!(linspace(0, 2, 1), vec![1]);
        assert_eq!(linspace(0, 2, 2), vec![0, 2]);
        assert_eq!(linspace(0, 2, 3), vec![0, 1, 2]);
        assert_eq!(linspace(0, 5, 3), vec![0, 3, 5]);
        assert_eq!(linspace(0, 100, 6), vec![0, 20, 40, 60, 80, 100]);
        assert_eq!(linspace(0, 100, 7), vec![0, 17, 33, 50, 67, 83, 100]);
        assert_eq!(linspace(100, 0, 7), vec![100, 83, 67, 50, 33, 17, 0]);
    }
}

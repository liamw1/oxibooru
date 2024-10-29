use num_traits::PrimInt;

pub fn linspace<T: PrimInt, const N: usize>(start: T, end: T) -> [T; N] {
    let start_f64 = start.to_f64().unwrap();
    let end_f64 = end.to_f64().unwrap();
    let num_f64 = N as f64;

    let mut arr = [T::zero(); N];
    match N {
        0 => (),
        1 => {
            let midpoint = 0.5 * start_f64 + 0.5 * end_f64;
            arr[0] = T::from(midpoint).unwrap();
        }
        _ => {
            let step = (end_f64 - start_f64) / (num_f64 - 1.0);
            for (i, item) in arr.iter_mut().enumerate() {
                let point = (start_f64 + i as f64 * step).round();
                *item = T::from(point).unwrap();
            }
        }
    }
    arr
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn create_linspace() {
        assert_eq!(linspace(0, 2), [0; 0]);
        assert_eq!(linspace(0, 2), [1]);
        assert_eq!(linspace(0, 2), [0, 2]);
        assert_eq!(linspace(0, 2), [0, 1, 2]);
        assert_eq!(linspace(0, 5), [0, 3, 5]);
        assert_eq!(linspace(0, 100), [0, 20, 40, 60, 80, 100]);
        assert_eq!(linspace(0, 100), [0, 17, 33, 50, 67, 83, 100]);
        assert_eq!(linspace(100, 0), [100, 83, 67, 50, 33, 17, 0]);
    }
}

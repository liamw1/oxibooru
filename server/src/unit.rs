use std::num::NonZeroU128;
use std::time::Duration;

pub fn format_duration(duration: Duration) -> String {
    const BASE: NonZeroU128 = NonZeroU128::new(1000).unwrap();
    format(duration.as_nanos(), BASE, TIME_PREFIXES, "s")
}

pub fn format_bytes(num_bytes: u64) -> String {
    const BASE: NonZeroU128 = NonZeroU128::new(1024).unwrap();
    format(u128::from(num_bytes), BASE, SI_PREFIXES, "B")
}

const TIME_PREFIXES: [&str; 4] = ["n", "μ", "m", ""];
const SI_PREFIXES: [&str; 11] = ["", "k", "M", "G", "T", "P", "E", "Z", "Y", "R", "Q"];

fn format<const N: usize>(num: u128, base: NonZeroU128, prefixes: [&str; N], unit: &str) -> String {
    const PRECISION: u32 = 1;

    let exp = num.checked_ilog(base.get()).unwrap_or(0);
    let prefix_index = std::cmp::min(usize::try_from(exp).unwrap_or(usize::MAX), N);
    let prefix = prefixes[prefix_index];

    let unit_size = base.saturating_pow(exp);
    if exp == 0 {
        let whole = num / unit_size;
        format!("{whole}{prefix}{unit}")
    } else {
        let scale = 10_u128.pow(PRECISION);
        let scaled = (num * scale + unit_size.get() / 2) / unit_size;

        let whole = scaled / scale;
        let fract = scaled % scale;
        let padding = usize::try_from(PRECISION).unwrap_or(0);
        format!("{whole}.{fract:0padding$}{prefix}{unit}")
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn format_base_1000() {
        const BASE: NonZeroU128 = NonZeroU128::new(1000).unwrap();

        assert_eq!(&format(0, BASE, SI_PREFIXES, "g"), "0g");
        assert_eq!(&format(1, BASE, SI_PREFIXES, "g"), "1g");
        assert_eq!(&format(6, BASE, SI_PREFIXES, "g"), "6g");
        assert_eq!(&format(15, BASE, SI_PREFIXES, "g"), "15g");
        assert_eq!(&format(854, BASE, SI_PREFIXES, "g"), "854g");
        assert_eq!(&format(1234, BASE, SI_PREFIXES, "g"), "1.2kg");
        assert_eq!(&format(1254, BASE, SI_PREFIXES, "g"), "1.3kg");
        assert_eq!(&format(1199090, BASE, SI_PREFIXES, "g"), "1.2Mg");
        assert_eq!(&format(19890900, BASE, SI_PREFIXES, "g"), "19.9Mg");
        assert_eq!(&format(19990900, BASE, SI_PREFIXES, "g"), "20.0Mg");
        assert_eq!(&format(377904867293476, BASE, SI_PREFIXES, "g"), "377.9Tg");
    }

    #[test]
    fn format_base_60() {
        const BASE: NonZeroU128 = NonZeroU128::new(60).unwrap();
        const UNITS: [&str; 3] = ["s", "m", "hr"];

        assert_eq!(&format(0, BASE, UNITS, ""), "0s");
        assert_eq!(&format(1, BASE, UNITS, ""), "1s");
        assert_eq!(&format(14, BASE, UNITS, ""), "14s");
        assert_eq!(&format(60, BASE, UNITS, ""), "1.0m");
        assert_eq!(&format(61, BASE, UNITS, ""), "1.0m");
        assert_eq!(&format(66, BASE, UNITS, ""), "1.1m");
        assert_eq!(&format(90, BASE, UNITS, ""), "1.5m");
        assert_eq!(&format(628, BASE, UNITS, ""), "10.5m");
        assert_eq!(&format(3600, BASE, UNITS, ""), "1.0hr");
        assert_eq!(&format(3660, BASE, UNITS, ""), "1.0hr");
        assert_eq!(&format(4500, BASE, UNITS, ""), "1.3hr");
        assert_eq!(&format(54060, BASE, UNITS, ""), "15.0hr");
        assert_eq!(&format(54179, BASE, UNITS, ""), "15.0hr");
        assert_eq!(&format(54180, BASE, UNITS, ""), "15.1hr");
    }
}

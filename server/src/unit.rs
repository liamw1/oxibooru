use serde::{Deserialize, Deserializer};
use std::borrow::Cow;
use std::fmt::Display;
use std::num::NonZeroU128;
use std::str::FromStr;
use std::time::Duration;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ByteCount(u128);

impl ByteCount {
    pub fn as_u64(self) -> u64 {
        u64::try_from(self.0).unwrap_or(u64::MAX)
    }

    pub fn as_usize(self) -> usize {
        usize::try_from(self.0).unwrap_or(usize::MAX)
    }
}

impl Display for ByteCount {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&format(self.0, SI_BASE, SI_PREFIXES, "B"))
    }
}

impl FromStr for ByteCount {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        const PREFIX_COUNT: u32 = SI_PREFIXES.len() as u32;

        let s = s.trim().strip_suffix('B').ok_or("missing unit `B`")?;
        let (digits, exp) = SI_PREFIXES
            .into_iter()
            .zip(0..PREFIX_COUNT)
            .rev()
            .find_map(|(c, i)| s.strip_suffix(c).map(|s| (s.trim_end(), i)))
            .unwrap_or((s, 0));
        if let Some(c) = digits.chars().next_back()
            && !c.is_ascii_digit()
            && c != '.'
        {
            return Err(format!("Unknown SI prefix `{c}`"));
        }

        let (whole, fract) = digits.split_once('.').unwrap_or((digits, ""));
        if whole.is_empty() && fract.is_empty() {
            return Err("Missing digits".into());
        }

        let whole = if whole.is_empty() {
            0_u128
        } else {
            whole.parse().map_err(|err| format!("{err}"))?
        };
        let unit = SI_BASE.get().pow(exp);
        let mut total = whole.checked_mul(unit).ok_or("Overflow")?;

        if !fract.is_empty() {
            let scale = 10_u128
                .checked_pow(fract.len() as u32)
                .ok_or("Too many fractional digits")?;
            let fract: u128 = fract.parse().map_err(|e| format!("{e}"))?;
            let contribution = fract
                .checked_mul(unit)
                .and_then(|n| n.checked_add(scale / 2))
                .ok_or("Overflow")?
                / scale;
            total = total.checked_add(contribution).ok_or("Overflow")?;
        }

        Ok(ByteCount(total))
    }
}

impl<'de> Deserialize<'de> for ByteCount {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        Cow::<str>::deserialize(deserializer).and_then(|s| s.parse().map_err(serde::de::Error::custom))
    }
}

pub fn format_duration(duration: Duration) -> String {
    format(duration.as_nanos(), SI_BASE, TIME_PREFIXES, "s")
}

const TIME_PREFIXES: [&str; 4] = ["n", "μ", "m", ""];
const SI_PREFIXES: [&str; 11] = ["", "k", "M", "G", "T", "P", "E", "Z", "Y", "R", "Q"];

const SI_BASE: NonZeroU128 = NonZeroU128::new(1000).unwrap();

fn format<const N: usize>(num: u128, base: NonZeroU128, prefixes: [&str; N], unit: &str) -> String {
    const PRECISION: u32 = 1;

    let exp = num.checked_ilog(base.get()).unwrap_or(0);
    let prefix_index = std::cmp::min(usize::try_from(exp).unwrap_or(usize::MAX), N.saturating_sub(1));
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
        assert_eq!(&format(0, SI_BASE, SI_PREFIXES, "g"), "0g");
        assert_eq!(&format(1, SI_BASE, SI_PREFIXES, "g"), "1g");
        assert_eq!(&format(6, SI_BASE, SI_PREFIXES, "g"), "6g");
        assert_eq!(&format(15, SI_BASE, SI_PREFIXES, "g"), "15g");
        assert_eq!(&format(854, SI_BASE, SI_PREFIXES, "g"), "854g");
        assert_eq!(&format(1234, SI_BASE, SI_PREFIXES, "g"), "1.2kg");
        assert_eq!(&format(1254, SI_BASE, SI_PREFIXES, "g"), "1.3kg");
        assert_eq!(&format(1199090, SI_BASE, SI_PREFIXES, "g"), "1.2Mg");
        assert_eq!(&format(19890900, SI_BASE, SI_PREFIXES, "g"), "19.9Mg");
        assert_eq!(&format(19990900, SI_BASE, SI_PREFIXES, "g"), "20.0Mg");
        assert_eq!(&format(377904867293476, SI_BASE, SI_PREFIXES, "g"), "377.9Tg");
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

    #[test]
    fn byte_count_from_str() {
        assert_eq!(ByteCount::from_str("8B").unwrap(), ByteCount(8));
        assert_eq!(ByteCount::from_str("777B").unwrap(), ByteCount(777));
        assert_eq!(ByteCount::from_str("1kB").unwrap(), ByteCount(1000));
        assert_eq!(ByteCount::from_str("55kB").unwrap(), ByteCount(55000));
        assert_eq!(ByteCount::from_str("100.7kB").unwrap(), ByteCount(100700));
        assert_eq!(ByteCount::from_str("10.1MB").unwrap(), ByteCount(10100000));
        assert_eq!(ByteCount::from_str("83.4545MB").unwrap(), ByteCount(83454500));
        assert_eq!(ByteCount::from_str("2GB").unwrap(), ByteCount(2 * 1000_u128.pow(3)));
        assert_eq!(ByteCount::from_str("7TB").unwrap(), ByteCount(7 * 1000_u128.pow(4)));
        assert_eq!(ByteCount::from_str("10QB").unwrap(), ByteCount(10 * 1000_u128.pow(10)));

        // Round fractional bytes
        assert_eq!(ByteCount::from_str("1.3B").unwrap(), ByteCount(1));
        assert_eq!(ByteCount::from_str("1.733333B").unwrap(), ByteCount(2));
        assert_eq!(ByteCount::from_str("1.23456kB").unwrap(), ByteCount(1235));
    }
}

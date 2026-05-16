use diesel::AsExpression;
use diesel::pg::Pg;
use diesel::serialize::{self, IsNull, Output, ToSql};
use diesel::sql_types::BigInt;
use serde::{Deserialize, Deserializer};
use std::borrow::Cow;
use std::convert::TryFrom;
use std::fmt::{Display, Formatter};
use std::io::Write;
use std::num::{NonZeroU128, ParseIntError, TryFromIntError};
use std::str::FromStr;
use std::time::Duration;
use thiserror::Error;

#[derive(Debug, Error)]
#[error(transparent)]
pub enum ParseByteCountError {
    #[error("Missing digits")]
    MissingDigits,
    #[error("Missing unit `B`")]
    MissingUnit,
    #[error("Overflow")]
    Overflow,
    ParseInt(#[from] ParseIntError),
    #[error("Too many digits")]
    TooManyDigits,
    #[error("Unknown SI prefix `{0}`")]
    UnknownPrefix(char),
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, AsExpression)]
#[diesel(sql_type = BigInt)]
pub struct ByteCount(u128);

impl TryFrom<i64> for ByteCount {
    type Error = TryFromIntError;
    fn try_from(value: i64) -> Result<Self, Self::Error> {
        u128::try_from(value).map(Self)
    }
}

impl TryFrom<ByteCount> for u64 {
    type Error = TryFromIntError;
    fn try_from(value: ByteCount) -> Result<Self, Self::Error> {
        u64::try_from(value.0)
    }
}

impl TryFrom<ByteCount> for usize {
    type Error = TryFromIntError;
    fn try_from(value: ByteCount) -> Result<Self, Self::Error> {
        usize::try_from(value.0)
    }
}

impl Display for ByteCount {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_str(&format(self.0, SI_BASE, SI_PREFIXES, "B"))
    }
}

impl FromStr for ByteCount {
    type Err = ParseByteCountError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        const PREFIX_COUNT: u32 = SI_PREFIXES.len() as u32;

        let s = s.trim().strip_suffix('B').ok_or(ParseByteCountError::MissingUnit)?;
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
            return Err(ParseByteCountError::UnknownPrefix(c));
        }

        let (whole, fract) = digits.split_once('.').unwrap_or((digits, ""));
        if whole.is_empty() && fract.is_empty() {
            return Err(ParseByteCountError::MissingDigits);
        }

        let whole = if whole.is_empty() { 0_u128 } else { whole.parse()? };
        let unit = SI_BASE.get().pow(exp);
        let mut total = whole.checked_mul(unit).ok_or(ParseByteCountError::Overflow)?;

        if !fract.is_empty() {
            let scale = 10_u128
                .checked_pow(fract.len() as u32)
                .ok_or(ParseByteCountError::TooManyDigits)?;
            let fract: u128 = fract.parse()?;
            let contribution = fract
                .checked_mul(unit)
                .and_then(|n| n.checked_add(scale / 2))
                .ok_or(ParseByteCountError::Overflow)?
                / scale;
            total = total.checked_add(contribution).ok_or(ParseByteCountError::Overflow)?;
        }

        Ok(ByteCount(total))
    }
}

impl<'de> Deserialize<'de> for ByteCount {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        Cow::<str>::deserialize(deserializer).and_then(|s| s.parse().map_err(serde::de::Error::custom))
    }
}

impl ToSql<BigInt, Pg> for ByteCount {
    fn to_sql<'a>(&'a self, out: &mut Output<'a, '_, Pg>) -> serialize::Result {
        let byte_count_i64 = i64::try_from(self.0)?;
        out.write_all(&byte_count_i64.to_be_bytes())?;
        Ok(IsNull::No)
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

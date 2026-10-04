use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::borrow::Cow;
use std::marker::PhantomData;
use std::ops::{BitOr, BitOrAssign, Index};
use std::str::FromStr;
use strum::IntoEnumIterator;

#[derive(Clone, Copy)]
pub struct Mask<F> {
    value: u64,
    phantom: PhantomData<F>,
}

impl<F> Mask<F>
where
    u64: From<F>,
{
    pub const fn all() -> Self {
        Self::from_u64(u64::MAX)
    }

    pub const fn none() -> Self {
        Self::from_u64(0)
    }

    pub fn toggle(&mut self, field: F) {
        self.value ^= 1 << u64::from(field);
    }

    const fn from_u64(value: u64) -> Self {
        Self {
            value,
            phantom: PhantomData,
        }
    }

    fn contains(&self, field: F) -> bool {
        self.value & (1 << u64::from(field)) != 0
    }
}

impl<F> Index<F> for Mask<F>
where
    u64: From<F>,
{
    type Output = bool;
    fn index(&self, index: F) -> &Self::Output {
        if self.contains(index) { &true } else { &false }
    }
}

impl<F> BitOr<F> for Mask<F>
where
    u64: From<F>,
{
    type Output = Self;
    fn bitor(self, rhs: F) -> Self::Output {
        Self::from_u64(self.value | (1 << u64::from(rhs)))
    }
}

impl<F> BitOrAssign<F> for Mask<F>
where
    u64: From<F>,
{
    fn bitor_assign(&mut self, rhs: F) {
        self.value |= 1 << u64::from(rhs);
    }
}

impl<F> From<&[F]> for Mask<F>
where
    F: Copy,
    u64: From<F>,
{
    fn from(value: &[F]) -> Self {
        value.iter().fold(Self::none(), |fields, &field| fields | field)
    }
}

impl<F, const N: usize> From<[F; N]> for Mask<F>
where
    F: Copy,
    u64: From<F>,
{
    fn from(value: [F; N]) -> Self {
        Self::from(value.as_slice())
    }
}

impl<F> FromStr for Mask<F>
where
    F: FromStr,
    u64: From<F>,
{
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if s.is_empty() {
            Ok(Self::none())
        } else {
            s.split(',').try_fold(Self::none(), |fields, field_str| {
                F::from_str(field_str.trim())
                    .map(|field| fields | field)
                    .map_err(|_| format!("Invalid field `{field_str}`"))
            })
        }
    }
}

impl<F> Serialize for Mask<F>
where
    F: IntoEnumIterator + Into<&'static str> + Copy,
    u64: From<F>,
{
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let fields: Vec<&str> = F::iter().filter(|&field| self[field]).map(Into::into).collect();
        serializer.serialize_str(&fields.join(","))
    }
}

impl<'de, F> Deserialize<'de> for Mask<F>
where
    F: FromStr,
    u64: From<F>,
{
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        match Option::<Cow<str>>::deserialize(deserializer)? {
            Some(field_list) => field_list.parse().map_err(serde::de::Error::custom),
            None => Ok(Self::from_u64(u64::MAX)),
        }
    }
}

pub struct Batcher<F> {
    enabled_fields: Mask<F>,
    batch_size: usize,
}

impl<F> Batcher<F>
where
    u64: From<F>,
{
    pub fn new(enabled_fields: Mask<F>, batch_size: usize) -> Self {
        Self {
            enabled_fields,
            batch_size,
        }
    }

    pub fn exec<T, E, G>(&self, field: F, mut function: G) -> Result<Vec<T>, E>
    where
        G: FnMut() -> Result<Vec<T>, E>,
    {
        Ok(if self.enabled_fields[field] {
            let results = function()?;
            assert_eq!(results.len(), self.batch_size);
            results
        } else {
            Vec::new()
        })
    }
}

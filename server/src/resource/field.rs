use serde::Deserialize;
use std::marker::PhantomData;
use std::ops::{BitOr, BitOrAssign, Index};
use std::str::FromStr;

#[derive(Clone, Copy)]
pub struct Mask<F> {
    value: u64,
    phantom: PhantomData<F>,
}

impl<F: Into<u64>> Mask<F> {
    pub const fn new() -> Self {
        Self::from_u64(0)
    }

    const fn from_u64(value: u64) -> Self {
        Self {
            value,
            phantom: PhantomData,
        }
    }

    fn contains(&self, field: F) -> bool {
        self.value & (1 << field.into()) != 0
    }
}

impl<F: Into<u64>> Index<F> for Mask<F> {
    type Output = bool;
    fn index(&self, index: F) -> &Self::Output {
        if self.contains(index) { &true } else { &false }
    }
}

impl<F: Into<u64>> BitOr<F> for Mask<F> {
    type Output = Self;
    fn bitor(self, rhs: F) -> Self::Output {
        Self::from_u64(self.value | (1 << rhs.into()))
    }
}

impl<F: Into<u64>> BitOrAssign<F> for Mask<F> {
    fn bitor_assign(&mut self, rhs: F) {
        self.value |= 1 << rhs.into();
    }
}

impl<F: Copy + Into<u64>> From<&[F]> for Mask<F> {
    fn from(value: &[F]) -> Self {
        value.iter().fold(Self::new(), |fields, &field| fields | field)
    }
}

impl<F: Copy + Into<u64>, const N: usize> From<[F; N]> for Mask<F> {
    fn from(value: [F; N]) -> Self {
        Self::from(value.as_slice())
    }
}

impl<'de, F: Into<u64> + FromStr> Deserialize<'de> for Mask<F> {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        if let Some(field_list) = Option::<String>::deserialize(deserializer)? {
            field_list.split(',').try_fold(Self::new(), |fields, field_str| {
                F::from_str(field_str)
                    .map(|field| fields | field)
                    .map_err(|_| serde::de::Error::custom(format!("invalid field `{field_str}`")))
            })
        } else {
            Ok(Self::from_u64(u64::MAX))
        }
    }
}

pub struct Batcher<F> {
    enabled_fields: Mask<F>,
    batch_size: usize,
}

impl<F: Into<u64>> Batcher<F> {
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

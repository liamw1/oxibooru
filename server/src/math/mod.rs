use std::num::TryFromIntError;

pub mod cartesian;
pub mod interval;
pub mod point;
pub mod rect;

/// Useful trait for converting from unsigned to signed integer in generic context.
pub trait SignedCast {
    type Signed;
    fn to_signed(self) -> Result<Self::Signed, TryFromIntError>;
}

impl SignedCast for u8 {
    type Signed = i8;
    fn to_signed(self) -> Result<Self::Signed, TryFromIntError> {
        i8::try_from(self)
    }
}

impl SignedCast for u16 {
    type Signed = i16;
    fn to_signed(self) -> Result<Self::Signed, TryFromIntError> {
        i16::try_from(self)
    }
}

impl SignedCast for u32 {
    type Signed = i32;
    fn to_signed(self) -> Result<Self::Signed, TryFromIntError> {
        i32::try_from(self)
    }
}

impl SignedCast for u64 {
    type Signed = i64;
    fn to_signed(self) -> Result<Self::Signed, TryFromIntError> {
        i64::try_from(self)
    }
}

impl SignedCast for usize {
    type Signed = isize;
    fn to_signed(self) -> Result<Self::Signed, TryFromIntError> {
        isize::try_from(self)
    }
}

/// Useful trait for converting from signed to unsigned integer in generic context.
pub trait UnsignedCast {
    type Unsigned;
    fn to_unsigned(self) -> Result<Self::Unsigned, TryFromIntError>;
}

impl UnsignedCast for i8 {
    type Unsigned = u8;
    fn to_unsigned(self) -> Result<Self::Unsigned, TryFromIntError> {
        u8::try_from(self)
    }
}

impl UnsignedCast for i16 {
    type Unsigned = u16;
    fn to_unsigned(self) -> Result<Self::Unsigned, TryFromIntError> {
        u16::try_from(self)
    }
}

impl UnsignedCast for i32 {
    type Unsigned = u32;
    fn to_unsigned(self) -> Result<Self::Unsigned, TryFromIntError> {
        u32::try_from(self)
    }
}

impl UnsignedCast for i64 {
    type Unsigned = u64;
    fn to_unsigned(self) -> Result<Self::Unsigned, TryFromIntError> {
        u64::try_from(self)
    }
}

impl UnsignedCast for isize {
    type Unsigned = usize;
    fn to_unsigned(self) -> Result<Self::Unsigned, TryFromIntError> {
        usize::try_from(self)
    }
}

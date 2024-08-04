pub trait ErrorKind {
    fn kind(&self) -> &'static str;
}

impl ErrorKind for std::env::VarError {
    fn kind(&self) -> &'static str {
        match self {
            Self::NotPresent => "NotPresent",
            Self::NotUnicode(_) => "NotUnicode",
        }
    }
}

impl ErrorKind for argon2::password_hash::errors::B64Error {
    fn kind(&self) -> &'static str {
        match self {
            Self::InvalidEncoding => "InvalidEncoding",
            Self::InvalidLength => "InvalidLength",
        }
    }
}

impl ErrorKind for argon2::password_hash::errors::InvalidValue {
    fn kind(&self) -> &'static str {
        match self {
            Self::InvalidChar(_) => "InvalidChar",
            Self::InvalidFormat => "InvalidFormat",
            Self::Malformed => "MalformedValue",
            Self::TooLong => "ValueTooLong",
            Self::TooShort => "ValueTooShort",
            _ => "UnknownArgonInvalidValue",
        }
    }
}

impl ErrorKind for argon2::password_hash::Error {
    fn kind(&self) -> &'static str {
        match self {
            Self::Algorithm => "UnsupportedAlgorithm",
            Self::B64Encoding(err) => err.kind(),
            Self::Crypto => "CryptoError",
            Self::OutputSize { .. } => "UnexpectedOutputSize",
            Self::ParamNameDuplicated => "ParamNameDuplicated",
            Self::ParamNameInvalid => "ParamNameInvalid",
            Self::ParamValueInvalid(err) => err.kind(),
            Self::ParamsMaxExceeded => "ParamsMaxExceeded",
            Self::Password => "InvalidPassword",
            Self::PhcStringField => "InvalidPhcStringField",
            Self::PhcStringTrailingData => "PhcStringTrailingData",
            Self::SaltInvalid(err) => err.kind(),
            Self::Version => "InvalidVersion",
            _ => "UnknownArgonError",
        }
    }
}

impl ErrorKind for crate::auth::HashError {
    fn kind(&self) -> &'static str {
        match self {
            Self::EnvVar(err) => err.kind(),
            Self::Hash(err) => err.kind(),
        }
    }
}

impl ErrorKind for diesel::result::DatabaseErrorKind {
    fn kind(&self) -> &'static str {
        match self {
            Self::CheckViolation => "CheckViolation",
            Self::ClosedConnection => "ClosedConnection",
            Self::ForeignKeyViolation => "ForeignKeyViolation",
            Self::NotNullViolation => "NotNullViolation",
            Self::ReadOnlyTransaction => "ReadOnlyTransaction",
            Self::SerializationFailure => "SerializationFailure",
            Self::UnableToSendCommand => "UnableToSendCommand",
            Self::UniqueViolation => "UniqueViolation",
            _ => "UnknownDatabaseError",
        }
    }
}

impl ErrorKind for diesel::result::Error {
    fn kind(&self) -> &'static str {
        match self {
            Self::AlreadyInTransaction => "AlreadyInTransaction",
            Self::BrokenTransactionManager => "BrokenTransactionManager",
            Self::DatabaseError(err, _) => err.kind(),
            Self::DeserializationError(_) => "DeserializationError",
            Self::InvalidCString(_) => "InvalidCString",
            Self::NotFound => "NotFound",
            Self::NotInTransaction => "NotInTransaction",
            Self::QueryBuilderError(_) => "QueryBuilderError",
            Self::RollbackErrorOnCommit { rollback_error, .. } => rollback_error.kind(),
            Self::RollbackTransaction => "RollbackTransaction",
            Self::SerializationError(_) => "SerializationError",
            _ => "UnknownQueryError",
        }
    }
}

impl ErrorKind for diesel::ConnectionError {
    fn kind(&self) -> &'static str {
        match self {
            Self::BadConnection(_) => "BadConnection",
            Self::CouldntSetupConfiguration(err) => err.kind(),
            Self::InvalidCString(_) => "InvalidCString",
            Self::InvalidConnectionUrl(_) => "InvalidConnectionUrl",
            _ => "UnknownDatabaseConnectionError",
        }
    }
}

impl ErrorKind for base64::DecodeError {
    fn kind(&self) -> &'static str {
        match self {
            Self::InvalidByte(..) => "InvalidByte",
            Self::InvalidLastSymbol(..) => "InvalidLastSymbol",
            Self::InvalidLength(_) => "InvalidLength",
            Self::InvalidPadding => "InvalidPadding",
        }
    }
}

impl ErrorKind for crate::auth::header::AuthenticationError {
    fn kind(&self) -> &'static str {
        match self {
            Self::FailedConnection(err) => err.kind(),
            Self::FailedQuery(err) => err.kind(),
            Self::InvalidAuthType => "InvalidAuthType",
            Self::InvalidEncoding(err) => err.kind(),
            Self::InvalidPassword => "InvalidPassword",
            Self::InvalidToken => "InvalidToken",
            Self::MalformedCredentials => "MalformedCredentials",
            Self::MalformedToken(_) => "MalformedToken",
            Self::Utf8Conversion(_) => "Utf8ConversionError",
        }
    }
}

impl ErrorKind for image::error::LimitErrorKind {
    fn kind(&self) -> &'static str {
        match self {
            Self::DimensionError => "DimensionLimitsExceeded",
            Self::InsufficientMemory => "OutOfMemory",
            Self::Unsupported { .. } => "UnsupportedImageDimensions",
            _ => "UnknownImageLimitError",
        }
    }
}

impl ErrorKind for image::error::ParameterErrorKind {
    fn kind(&self) -> &'static str {
        match self {
            Self::DimensionMismatch => "DimensionMismatch",
            Self::FailedAlready => "FailedAlready",
            Self::Generic(_) => "GenericError",
            Self::NoMoreData => "NoMoreData",
            _ => "UnknownImageParameterError",
        }
    }
}

impl ErrorKind for image::error::UnsupportedErrorKind {
    fn kind(&self) -> &'static str {
        match self {
            Self::Color(_) => "UnsupportedColor",
            Self::Format(_) => "UnsupportedFormat",
            Self::GenericFeature(_) => "UnsupportedFeature",
            _ => "UnknownImageUnsupportedError",
        }
    }
}

impl ErrorKind for image::ImageError {
    fn kind(&self) -> &'static str {
        match self {
            Self::Decoding(_) => "FailedDecoding",
            Self::Encoding(_) => "FailedEncoding",
            Self::IoError(_) => "IOError",
            Self::Limits(err) => err.kind().kind(),
            Self::Parameter(err) => err.kind().kind(),
            Self::Unsupported(err) => err.kind().kind(),
        }
    }
}

impl ErrorKind for std::num::IntErrorKind {
    fn kind(&self) -> &'static str {
        match self {
            Self::Empty => "EmptyValue",
            Self::InvalidDigit => "InvalidDigit",
            Self::PosOverflow => "PositiveOverflow",
            Self::NegOverflow => "NegativeOverflow",
            Self::Zero => "Zero",
            _ => "UnknownIntParseError",
        }
    }
}

impl ErrorKind for crate::search::TimeParsingError {
    fn kind(&self) -> &'static str {
        match self {
            Self::TooFewArgs => "TooFewArgs",
            Self::TooManyArgs => "TooManyArgs",
            Self::NotAnInteger(err) => err.kind().kind(),
            Self::OutOfRange(_) => "OutOfRange",
        }
    }
}

impl ErrorKind for crate::search::Error {
    fn kind(&self) -> &'static str {
        match self {
            Self::ParseFailed(_) => "SearchParseFailed",
            Self::InvalidTime(err) => err.kind(),
            Self::NotLoggedIn => "NotLoggedIn",
        }
    }
}

impl ErrorKind for crate::api::Error {
    fn kind(&self) -> &'static str {
        match self {
            Self::BadExtension(_) => "BadExtension",
            Self::BadHash(err) => err.kind(),
            Self::BadHeader(_) => "BadHeader",
            Self::BadMultiPartForm => "BadMultiPartForm",
            Self::ContentTypeMismatch => "ContentTypeMismatch",
            Self::CyclicDependency => "CyclicDependency",
            Self::DeleteDefault => "DeleteDefault",
            Self::ExpressionFailsRegex => "ExpressionFailsRegex",
            Self::FailedAuthentication(err) => err.kind(),
            Self::FailedConnection(err) => err.kind(),
            Self::FailedQuery(err) => err.kind(),
            Self::FromStrError(_) => "FromStrError",
            Self::InsufficientPrivileges => "InsufficientPrivileges",
            Self::ImageError(err) => err.kind(),
            Self::IoError(_) => "IOError",
            Self::NoNamesGiven => "NoNamesGiven",
            Self::NotAnInteger(err) => err.kind().kind(),
            Self::NotLoggedIn => "NotLoggedIn",
            Self::ResourceModified => "ResourceModified",
            Self::SearchError(err) => err.kind(),
            Self::SelfMerge => "SelfMerge",
            Self::Utf8Conversion(_) => "Utf8ConversionError",
            Self::WarpError(_) => "WarpError",
        }
    }
}

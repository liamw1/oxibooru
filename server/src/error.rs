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

impl ErrorKind for std::io::ErrorKind {
    fn kind(&self) -> &'static str {
        match self {
            Self::NotFound => "NotFound",
            Self::PermissionDenied => "PermissionDenied",
            Self::ConnectionRefused => "ConnectionRefused",
            Self::ConnectionReset => "ConnectionReset",
            Self::ConnectionAborted => "ConnectionAborted",
            Self::NotConnected => "NotConnected",
            Self::AddrInUse => "AddrInUse",
            Self::AddrNotAvailable => "AddrNotAvailable",
            Self::BrokenPipe => "BrokenPipe",
            Self::AlreadyExists => "AlreadyExists",
            Self::WouldBlock => "WouldBlock",
            Self::InvalidInput => "InvalidInput",
            Self::InvalidData => "InvalidData",
            Self::TimedOut => "TimedOut",
            Self::WriteZero => "WriteZero",
            Self::Interrupted => "Interrupted",
            Self::Unsupported => "Unsupported",
            Self::UnexpectedEof => "UnexpectedEof",
            Self::OutOfMemory => "OutOfMemory",
            Self::Other => "OtherIOError",
            _ => "UnknownIOError",
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
            Self::FailedConnection(_) => "FailedConnection",
            Self::FailedQuery(err) => err.kind(),
            Self::InvalidAuthType => "InvalidAuthType",
            Self::InvalidEncoding(err) => err.kind(),
            Self::InvalidToken => "InvalidToken",
            Self::MalformedCredentials => "MalformedCredentials",
            Self::MalformedToken(_) => "MalformedToken",
            Self::UsernamePasswordMismatch => "UsernamePasswordMismatch",
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
            Self::IoError(err) => err.kind().kind(),
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
            Self::InvalidSort => "InvalidSort",
            Self::NotLoggedIn => "NotLoggedIn",
        }
    }
}

impl ErrorKind for video_rs::ffmpeg::Error {
    fn kind(&self) -> &'static str {
        match self {
            Self::Bug => "FFmpegBug1",
            Self::Bug2 => "FFmpegBug2",
            Self::Unknown => "FFmpegUnknown",
            Self::Experimental => "FFmpegExperimental",
            Self::BufferTooSmall => "FFmpegBufferTooSmall",
            Self::Eof => "FFmpegEof",
            Self::Exit => "FFmpegExit",
            Self::External => "FFmpegExternal",
            Self::InvalidData => "FFmpegInvalidData",
            Self::PatchWelcome => "FFmpegPatchWelcome",
            Self::InputChanged => "FFmpegInputChanged",
            Self::OutputChanged => "FFmpegOutputChanged",
            Self::BsfNotFound => "FFmpegBsfNotFound",
            Self::DecoderNotFound => "FFmpegDecoderNotFound",
            Self::DemuxerNotFound => "FFmpegDemuxerNotFound",
            Self::EncoderNotFound => "FFmpegEncoderNotFound",
            Self::OptionNotFound => "FFmpegOptionNotFound",
            Self::MuxerNotFound => "FFmpegMuxerNotFound",
            Self::FilterNotFound => "FFmpegFilterNotFound",
            Self::ProtocolNotFound => "FFmpegProtocolNotFound",
            Self::StreamNotFound => "FFmpegStreamNotFound",
            Self::HttpBadRequest => "FFmpegHttpBadRequest",
            Self::HttpUnauthorized => "FFmpegHttpUnauthorized",
            Self::HttpForbidden => "FFmpegHttpForbidden",
            Self::HttpNotFound => "FFmpegHttpNotFound",
            Self::HttpOther4xx => "FFmpegHttpOther4xx",
            Self::HttpServerError => "FFmpegHttpServerError",
            Self::Other { .. } => "FFmpegPosixError",
        }
    }
}

impl ErrorKind for video_rs::Error {
    fn kind(&self) -> &'static str {
        match self {
            Self::ReadExhausted => "ReadExhausted",
            Self::DecodeExhausted => "DecodeExhausted",
            Self::WriteRetryLimitReached => "WriteRetryLimitReached",
            Self::InvalidFrameFormat => "InvalidFrameFormat",
            Self::InvalidExtraData => "InvalidExtraData",
            Self::MissingCodecParameters => "MissingCodecParameters",
            Self::UnsupportedCodecParameterSets => "UnsupportedCodecParameterSets",
            Self::InvalidResizeParameters => "InvalidResizeParameters",
            Self::UninitializedCodec => "UninitializedCodec",
            Self::UnsupportedCodecHardwareAccelerationDeviceType => "UnsupportedCodecHardwareAccelerationDeviceType",
            Self::BackendError(err) => err.kind(),
        }
    }
}

impl ErrorKind for lettre::address::AddressError {
    fn kind(&self) -> &'static str {
        match self {
            Self::MissingParts => "EmailAddressMissingParts",
            Self::Unbalanced => "EmailAddressUnbalanced",
            Self::InvalidUser => "EmailAddressInvalidUser",
            Self::InvalidDomain => "EmailAddressInvalidDomain",
            Self::InvalidInput => "EmailAddressInvalidInput",
            _ => "UnknownEmailAddressError",
        }
    }
}

impl ErrorKind for lettre::error::Error {
    fn kind(&self) -> &'static str {
        match self {
            Self::MissingFrom => "EmailMissingForm",
            Self::MissingTo => "EmailMissingTo",
            Self::TooManyFrom => "EmailTooManyFrom",
            Self::EmailMissingAt => "EmailMissingAt",
            Self::EmailMissingLocalPart => "EmailMissingLocalPart",
            Self::EmailMissingDomain => "EmailMissingDomain",
            Self::CannotParseFilename => "EmailCannotParseFilename",
            Self::Io(err) => err.kind().kind(),
            Self::NonAsciiChars => "EmailNonAsciiChars",
        }
    }
}

impl ErrorKind for serde_json::error::Category {
    fn kind(&self) -> &'static str {
        match self {
            Self::Io => "SerdeJsonIO",
            Self::Syntax => "SerdeJsonSyntax",
            Self::Data => "SerdeJsonData",
            Self::Eof => "SerdeJsonEOF",
        }
    }
}

impl ErrorKind for swf::error::Error {
    fn kind(&self) -> &'static str {
        match self {
            Self::Avm1ParseError { .. } => "SwfAvm1ParseError",
            Self::InvalidData(_) => "SwfInvalidData",
            Self::SwfParseError { .. } => "SwfParseError",
            Self::IoError(_) => "SwfIoError",
            Self::Unsupported(_) => "SwfUnsupported",
        }
    }
}

impl ErrorKind for crate::api::Error {
    fn kind(&self) -> &'static str {
        match self {
            Self::BadExtension(_) => "BadExtension",
            Self::BadHash(err) => err.kind(),
            Self::BadIncomingHeader(_) => "BadIncomingHeader",
            Self::BadResponseHeader(_) => "BadResponseHeader",
            Self::ContentTypeMismatch(..) => "ContentTypeMismatch",
            Self::CyclicDependency(_) => "CyclicDependency",
            Self::DeleteDefault(_) => "DeleteDefault",
            Self::EmptySwf => "EmptySwf",
            Self::EmptyVideo => "EmptyVideo",
            Self::ExpressionFailsRegex(_) => "ExpressionFailsRegex",
            Self::FailedAuthentication(err) => err.kind(),
            Self::FailedConnection(_) => "FailedConnection",
            Self::FailedEmailTransport(_) => "FailedEmailTransport",
            Self::FailedQuery(err) => err.kind(),
            Self::FromStr(_) => "FromStrError",
            Self::InsufficientPrivileges => "InsufficientPrivileges",
            Self::InvalidEmailAddress(err) => err.kind(),
            Self::InvalidEmail(err) => err.kind(),
            Self::InvalidHeader(_) => "InvalidHeader",
            Self::InvalidMetadataType => "InvalidMetadataType",
            Self::InvalidUserRank => "InvalidUserRank",
            Self::Image(err) => err.kind(),
            Self::JsonSerialization(err) => err.classify().kind(),
            Self::NoEmail => "NoEmail",
            Self::MissingContent(_) => "MissingContent",
            Self::MissingContentType => "MissingContentType",
            Self::MissingFormData => "MissingFormData",
            Self::MissingMetadata => "MissingMetadata",
            Self::MissingSmtpInfo => "MissingSmtpInfo",
            Self::NoNamesGiven(_) => "NoNamesGiven",
            Self::NotAnInteger(err) => err.kind().kind(),
            Self::NotFound(_) => "NotFound",
            Self::NotLoggedIn => "NotLoggedIn",
            Self::Request(_) => "RequestError",
            Self::ResourceModified => "ResourceModified",
            Self::Search(err) => err.kind(),
            Self::SelfMerge(_) => "SelfMerge",
            Self::StdIo(err) => err.kind().kind(),
            Self::SwfDecoding(err) => err.kind(),
            Self::UnauthorizedPasswordReset => "UnauthorizedPasswordReset",
            Self::Utf8Conversion(_) => "Utf8ConversionError",
            Self::VideoDecoding(err) => err.kind(),
            Self::Warp(_) => "WarpError",
        }
    }
}

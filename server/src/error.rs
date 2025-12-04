pub trait ErrorKind {
    fn kind(&self) -> &'static str;
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
            Self::InvalidChar(_) => "InvalidCharacter",
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
            Self::ParamsMaxExceeded => "ParamsMaxExceeded",
            Self::ParamValueInvalid(err) | Self::SaltInvalid(err) => err.kind(),
            Self::Password => "InvalidPassword",
            Self::PhcStringField => "InvalidPhcStringField",
            Self::PhcStringTrailingData => "PhcStringTrailingData",
            Self::Version => "InvalidVersion",
            _ => "UnknownArgonError",
        }
    }
}

impl ErrorKind for axum::extract::multipart::MultipartRejection {
    fn kind(&self) -> &'static str {
        match self {
            Self::InvalidBoundary(_) => "InvalidBoundary",
            _ => "UnknownMultipartRejectionError",
        }
    }
}

impl ErrorKind for axum::extract::path::ErrorKind {
    fn kind(&self) -> &'static str {
        match self {
            Self::WrongNumberOfParameters { .. } => "WrongNumberOfPathParameters",
            Self::ParseErrorAtKey { .. } => "PathParseErrorAtKey",
            Self::ParseErrorAtIndex { .. } => "PathParseErrorAtIndex",
            Self::ParseError { .. } => "PathParseError",
            Self::InvalidUtf8InPathParam { .. } => "InvalidUtf8InPathParam",
            Self::UnsupportedType { .. } => "UnsupportedPathType",
            Self::DeserializeError { .. } => "PathDeserializeError",
            Self::Message(_) => "OtherPathError",
            _ => "UnknownPathDeserializeError",
        }
    }
}

impl ErrorKind for axum::extract::rejection::JsonRejection {
    fn kind(&self) -> &'static str {
        match self {
            Self::JsonDataError(_) => "JsonDataError",
            Self::JsonSyntaxError(_) => "JsonSyntaxError",
            Self::MissingJsonContentType(_) => "MissingJsonContentType",
            Self::BytesRejection(_) => "BytesRejection",
            _ => "UnknownJsonRejectionError",
        }
    }
}

impl ErrorKind for axum::extract::rejection::PathRejection {
    fn kind(&self) -> &'static str {
        match self {
            Self::FailedToDeserializePathParams(err) => err.kind().kind(),
            Self::MissingPathParams(_) => "MissingPathParams",
            _ => "UnknownPathRejectionError",
        }
    }
}

impl ErrorKind for axum::extract::rejection::QueryRejection {
    fn kind(&self) -> &'static str {
        match self {
            Self::FailedToDeserializeQueryString(_) => "FailedToDeserializeQueryString",
            _ => "UnknownQueryRejectionError",
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

impl ErrorKind for crate::model::enums::ResourceType {
    fn kind(&self) -> &'static str {
        match self {
            Self::Comment => "CommentNotFound",
            Self::Pool => "PoolNotFound",
            Self::PoolCategory => "PoolCategoryNotFound",
            Self::Post => "PostNotFound",
            Self::Tag | Self::TagImplication | Self::TagSuggestion => "TagNotFound",
            Self::TagCategory => "TagCategoryNotFound",
            Self::User => "UserNotFound",
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
            Self::NotFound => "RowNotFound",
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

impl ErrorKind for image::error::LimitErrorKind {
    fn kind(&self) -> &'static str {
        match self {
            Self::DimensionError => "DimensionLimitsExceeded",
            Self::InsufficientMemory => "InsufficientMemory",
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
            Self::Io => "JsonIoError",
            Self::Syntax => "JsonInvalidSyntax",
            Self::Data => "JsonInvalidData",
            Self::Eof => "JsonUnexpectedEOF",
        }
    }
}

impl ErrorKind for std::env::VarError {
    fn kind(&self) -> &'static str {
        match self {
            Self::NotPresent => "EnvironmentVariableNotPresent",
            Self::NotUnicode(_) => "EnvironmentVariableNotUnicode",
        }
    }
}

impl ErrorKind for std::io::ErrorKind {
    fn kind(&self) -> &'static str {
        match self {
            Self::NotFound => "FileNotFound",
            Self::PermissionDenied => "PermissionDenied",
            Self::ConnectionRefused => "ConnectionRefused",
            Self::ConnectionReset => "ConnectionReset",
            Self::HostUnreachable => "HostUnreachable",
            Self::NetworkUnreachable => "NetworkUnreachable",
            Self::ConnectionAborted => "ConnectionAborted",
            Self::NotConnected => "NotConnected",
            Self::AddrInUse => "AddressInUse",
            Self::AddrNotAvailable => "AddressNotAvailable",
            Self::NetworkDown => "NetworkDown",
            Self::BrokenPipe => "BrokenPipe",
            Self::AlreadyExists => "FileAlreadyExists",
            Self::WouldBlock => "WouldBlock",
            Self::NotADirectory => "NotADirectory",
            Self::IsADirectory => "IsADirectory",
            Self::DirectoryNotEmpty => "DirectoryNotEmpty",
            Self::ReadOnlyFilesystem => "ReadOnlyFilesystem",
            Self::StaleNetworkFileHandle => "StaleNetworkFileHandle",
            Self::InvalidInput => "InvalidInput",
            Self::InvalidData => "InvalidData",
            Self::TimedOut => "TimedOut",
            Self::WriteZero => "WriteZero",
            Self::StorageFull => "StorageFull",
            Self::NotSeekable => "NotSeekable",
            Self::QuotaExceeded => "QuotaExceeded",
            Self::FileTooLarge => "FileTooLarge",
            Self::ResourceBusy => "ResourceBusy",
            Self::ExecutableFileBusy => "ExecutableFileBusy",
            Self::Deadlock => "Deadlock",
            Self::CrossesDevices => "CrossesDevices",
            Self::TooManyLinks => "TooManyLinks",
            Self::InvalidFilename => "InvalidFilename",
            Self::ArgumentListTooLong => "ArgumentListTooLong",
            Self::Interrupted => "Interrupted",
            Self::Unsupported => "Unsupported",
            Self::UnexpectedEof => "UnexpectedEof",
            Self::OutOfMemory => "OutOfMemory",
            Self::Other => "OtherIoError",
            _ => "UnknownIoError",
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
            Self::Zero => "ZeroNotAllowed",
            _ => "UnknownIntParseError",
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

impl ErrorKind for crate::api::ApiError {
    fn kind(&self) -> &'static str {
        match self {
            Self::ContentTypeMismatch(..) => "ContentTypeMismatch",
            Self::CyclicDependency(_) => "CyclicDependency",
            Self::DeleteDefault(_) => "DeleteDefault",
            Self::EmptySwf => "EmptySwf",
            Self::EmptyVideo => "EmptyVideo",
            Self::ExpressionFailsRegex(..) => "ExpressionFailsRegex",
            Self::FailedAuthentication(err) => err.kind(),
            Self::FailedConnection(_) => "FailedConnection",
            Self::FailedEmailTransport(_) => "FailedEmailTransport",
            Self::FailedQuery(err) => err.kind(),
            Self::FromStr(_) => "FromStrError",
            Self::HeaderDeserialization(_) => "HeaderDeserialization",
            Self::InsufficientPrivileges => "InsufficientPrivileges",
            Self::InvalidEmailAddress(err) => err.kind(),
            Self::InvalidEmail(err) => err.kind(),
            Self::InvalidHeader(_) => "InvalidHeader",
            Self::InvalidSort => "InvalidSort",
            Self::InvalidTime(err) => err.kind(),
            Self::InvalidUserRank => "InvalidUserRank",
            Self::Image(err) => err.kind(),
            Self::JsonRejection(err) => err.kind(),
            Self::JsonSerialization(err) => err.classify().kind(),
            Self::NoEmail => "NoEmail",
            Self::MissingContent(_) => "MissingContent",
            Self::MissingContentType => "MissingContentType",
            Self::MissingFormData => "MissingFormData",
            Self::MissingMetadata => "MissingMetadata",
            Self::MissingSmtpInfo => "MissingSmtpInfo",
            Self::Multipart(_) => "MultipartError",
            Self::MultipartRejection(err) => err.kind(),
            Self::NoNamesGiven(_) => "NoNamesGiven",
            Self::NotAnInteger(err) => err.kind().kind(),
            Self::NotFound(err) => err.kind(),
            Self::NotLoggedIn => "NotLoggedIn",
            Self::Password(err) => err.kind(),
            Self::PathRejection(err) => err.kind(),
            Self::QueryRejection(err) => err.kind(),
            Self::Request(_) => "RequestError",
            Self::ResourceModified => "ResourceModified",
            Self::SelfMerge(_) => "SelfMerge",
            Self::StdIo(err) => err.kind().kind(),
            Self::SwfDecoding(err) => err.kind(),
            Self::TooMany(_) => "TooMany",
            Self::UnimplementedFrameFormat(_) => "UnimplementedFrameFormat",
            Self::UnauthorizedPasswordReset => "UnauthorizedPasswordReset",
            Self::UnsupportedExtension(_) => "UnsupportedExtension",
            Self::VideoDecoding(err) => err.kind(),
        }
    }
}

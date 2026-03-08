use serde::Serialize;
use utoipa::ToSchema;

#[derive(Serialize, ToSchema)]
pub enum ErrorName {
    AddressInUse,
    AddressNotAvailable,
    AlreadyInTransaction,
    ArgumentListTooLong,
    BadConnection,
    BrokenPipe,
    BrokenTransactionManager,
    BytesRejection,
    CheckViolation,
    ClosedConnection,
    CommentNotFound,
    ConnectionAborted,
    ConnectionRefused,
    ConnectionReset,
    ContentTypeMismatch,
    CrossesDevices,
    CryptoError,
    CyclicDependency,
    Deadlock,
    DecodeExhausted,
    DeleteDefault,
    DeserializationError,
    DimensionLimitsExceeded,
    DimensionMismatch,
    DirectoryNotEmpty,
    DuplicatePost,
    EmailAddressInvalidDomain,
    EmailAddressInvalidInput,
    EmailAddressInvalidUser,
    EmailAddressMissingParts,
    EmailAddressUnbalanced,
    EmailCannotParseFilename,
    EmailMissingAt,
    EmailMissingDomain,
    EmailMissingForm,
    EmailMissingLocalPart,
    EmailMissingTo,
    EmailNonAsciiChars,
    EmailTooManyFrom,
    EmptySwf,
    EmptyValue,
    EmptyVideo,
    EnvironmentVariableNotPresent,
    EnvironmentVariableNotUnicode,
    ExecutableFileBusy,
    ExpiredToken,
    ExpressionFailsRegex,
    FailedAlready,
    FailedConnection,
    FailedDecoding,
    FailedEmailTransport,
    FailedEncoding,
    FailedToDeserializeQueryString,
    FFmpegBsfNotFound,
    FFmpegBufferTooSmall,
    FFmpegBug1,
    FFmpegBug2,
    FFmpegDecoderNotFound,
    FFmpegDemuxerNotFound,
    FFmpegEncoderNotFound,
    FFmpegEof,
    FFmpegExit,
    FFmpegExperimental,
    FFmpegExternal,
    FFmpegFilterNotFound,
    FFmpegHttpBadRequest,
    FFmpegHttpForbidden,
    FFmpegHttpNotFound,
    FFmpegHttpOther4xx,
    FFmpegHttpServerError,
    FFmpegHttpUnauthorized,
    FFmpegInputChanged,
    FFmpegInvalidData,
    FFmpegMuxerNotFound,
    FFmpegOptionNotFound,
    FFmpegOutputChanged,
    FFmpegPatchWelcome,
    FFmpegPosixError,
    FFmpegProtocolNotFound,
    FFmpegStreamNotFound,
    FFmpegUnknown,
    FileAlreadyExists,
    FileNotFound,
    FileTooLarge,
    ForeignKeyViolation,
    FromStrError,
    GenericImageError,
    HeaderDeserialization,
    HostUnreachable,
    InsufficientMemory,
    InsufficientPrivileges,
    Interrupted,
    InvalidAuthType,
    InvalidBoundary,
    InvalidByte,
    InvalidCharacter,
    InvalidConnectionUrl,
    InvalidCString,
    InvalidData,
    InvalidDigit,
    InvalidEncoding,
    InvalidExtraData,
    InvalidFilename,
    InvalidFormat,
    InvalidFrameFormat,
    InvalidHeader,
    InvalidInput,
    InvalidLastSymbol,
    InvalidLength,
    InvalidPadding,
    InvalidPassword,
    InvalidPhcStringField,
    InvalidResizeParameters,
    InvalidSort,
    InvalidUploadToken,
    InvalidUserRank,
    InvalidUtf8InPathParam,
    InvalidVersion,
    IsADirectory,
    JsonDataError,
    JsonInvalidData,
    JsonInvalidSyntax,
    JsonIoError,
    JsonSyntaxError,
    JsonUnexpectedEOF,
    MalformedCredentials,
    MalformedToken,
    MalformedValue,
    MissingCodecParameters,
    MissingContent,
    MissingContentType,
    MissingFormData,
    MissingJsonContentType,
    MissingMetadata,
    MissingPathParams,
    MissingSmtpInfo,
    MultipartError,
    NegativeOverflow,
    NetworkDown,
    NetworkUnreachable,
    NoEmail,
    NoMoreData,
    NoNamesGiven,
    NotADirectory,
    NotConnected,
    NotInTransaction,
    NotLoggedIn,
    NotNullViolation,
    NotSeekable,
    OtherIoError,
    OtherPathError,
    OutOfMemory,
    OutOfRange,
    ParamNameDuplicated,
    ParamNameInvalid,
    ParamsMaxExceeded,
    PathDeserializeError,
    PathParseError,
    PathParseErrorAtIndex,
    PathParseErrorAtKey,
    PermissionDenied,
    PhcStringTrailingData,
    PoolCategoryNameAlreadyExists,
    PoolCategoryNotFound,
    PoolNameAlreadyExists,
    PoolNotFound,
    PoolPostAlreadyExists,
    PositiveOverflow,
    PostAlreadyFeatured,
    PostNotFound,
    PostRelationAlreadyExists,
    QueryBuilderError,
    QuotaExceeded,
    ReadExhausted,
    ReadOnlyFilesystem,
    ReadOnlyTransaction,
    RequestError,
    ResourceBusy,
    ResourceHidden,
    ResourceModified,
    RollbackTransaction,
    RowNotFound,
    SelfMerge,
    SerializationError,
    SerializationFailure,
    StaleNetworkFileHandle,
    StorageFull,
    SwfAvm1ParseError,
    SwfInvalidData,
    SwfIoError,
    SwfParseError,
    SwfUnsupported,
    TagCategoryNameAlreadyExists,
    TagCategoryNotFound,
    TagNameAlreadyExists,
    TagNotFound,
    TimedOut,
    TooFewArgs,
    TooManyArgs,
    TooManyLinks,
    UnableToSendCommand,
    UnauthorizedPasswordReset,
    UnexpectedEof,
    UnexpectedOutputSize,
    UnimplementedFrameFormat,
    UninitializedCodec,
    UniqueViolation,
    UnknownArgonError,
    UnknownArgonInvalidValue,
    UnknownDatabaseConnectionError,
    UnknownDatabaseError,
    UnknownEmailAddressError,
    UnknownImageLimitError,
    UnknownImageParameterError,
    UnknownImageUnsupportedError,
    UnknownIntParseError,
    UnknownIoError,
    UnknownJsonRejectionError,
    UnknownMultipartRejectionError,
    UnknownPathDeserializeError,
    UnknownPathRejectionError,
    UnknownQueryError,
    UnknownQueryRejectionError,
    Unsupported,
    UnsupportedAlgorithm,
    UnsupportedCodecHardwareAccelerationDeviceType,
    UnsupportedCodecParameterSets,
    UnsupportedColor,
    UnsupportedExtension,
    UnsupportedFeature,
    UnsupportedFormat,
    UnsupportedImageDimensions,
    UnsupportedPathType,
    UserEmailAlreadyExists,
    UserNameAlreadyExists,
    UsernamePasswordMismatch,
    UsernameTokenMismatch,
    UserNotFound,
    UserTokenNotFound,
    Utf8ConversionError,
    ValueTooLong,
    ValueTooShort,
    WouldBlock,
    WriteRetryLimitReached,
    WriteZero,
    WrongNumberOfPathParameters,
    ZeroNotAllowed,
}

pub trait ErrorKind {
    fn kind(&self) -> ErrorName;
}

impl ErrorKind for argon2::password_hash::errors::B64Error {
    fn kind(&self) -> ErrorName {
        match self {
            Self::InvalidEncoding => ErrorName::InvalidEncoding,
            Self::InvalidLength => ErrorName::InvalidLength,
        }
    }
}

impl ErrorKind for argon2::password_hash::errors::InvalidValue {
    fn kind(&self) -> ErrorName {
        match self {
            Self::InvalidChar(_) => ErrorName::InvalidCharacter,
            Self::InvalidFormat => ErrorName::InvalidFormat,
            Self::Malformed => ErrorName::MalformedValue,
            Self::TooLong => ErrorName::ValueTooLong,
            Self::TooShort => ErrorName::ValueTooShort,
            _ => ErrorName::UnknownArgonInvalidValue,
        }
    }
}

impl ErrorKind for argon2::password_hash::Error {
    fn kind(&self) -> ErrorName {
        match self {
            Self::Algorithm => ErrorName::UnsupportedAlgorithm,
            Self::B64Encoding(err) => err.kind(),
            Self::Crypto => ErrorName::CryptoError,
            Self::OutputSize { .. } => ErrorName::UnexpectedOutputSize,
            Self::ParamNameDuplicated => ErrorName::ParamNameDuplicated,
            Self::ParamNameInvalid => ErrorName::ParamNameInvalid,
            Self::ParamsMaxExceeded => ErrorName::ParamsMaxExceeded,
            Self::ParamValueInvalid(err) | Self::SaltInvalid(err) => err.kind(),
            Self::Password => ErrorName::InvalidPassword,
            Self::PhcStringField => ErrorName::InvalidPhcStringField,
            Self::PhcStringTrailingData => ErrorName::PhcStringTrailingData,
            Self::Version => ErrorName::InvalidVersion,
            _ => ErrorName::UnknownArgonError,
        }
    }
}

impl ErrorKind for axum::extract::multipart::MultipartRejection {
    fn kind(&self) -> ErrorName {
        match self {
            Self::InvalidBoundary(_) => ErrorName::InvalidBoundary,
            _ => ErrorName::UnknownMultipartRejectionError,
        }
    }
}

impl ErrorKind for axum::extract::path::ErrorKind {
    fn kind(&self) -> ErrorName {
        match self {
            Self::WrongNumberOfParameters { .. } => ErrorName::WrongNumberOfPathParameters,
            Self::ParseErrorAtKey { .. } => ErrorName::PathParseErrorAtKey,
            Self::ParseErrorAtIndex { .. } => ErrorName::PathParseErrorAtIndex,
            Self::ParseError { .. } => ErrorName::PathParseError,
            Self::InvalidUtf8InPathParam { .. } => ErrorName::InvalidUtf8InPathParam,
            Self::UnsupportedType { .. } => ErrorName::UnsupportedPathType,
            Self::DeserializeError { .. } => ErrorName::PathDeserializeError,
            Self::Message(_) => ErrorName::OtherPathError,
            _ => ErrorName::UnknownPathDeserializeError,
        }
    }
}

impl ErrorKind for axum::extract::rejection::JsonRejection {
    fn kind(&self) -> ErrorName {
        match self {
            Self::JsonDataError(_) => ErrorName::JsonDataError,
            Self::JsonSyntaxError(_) => ErrorName::JsonSyntaxError,
            Self::MissingJsonContentType(_) => ErrorName::MissingJsonContentType,
            Self::BytesRejection(_) => ErrorName::BytesRejection,
            _ => ErrorName::UnknownJsonRejectionError,
        }
    }
}

impl ErrorKind for axum::extract::rejection::PathRejection {
    fn kind(&self) -> ErrorName {
        match self {
            Self::FailedToDeserializePathParams(err) => err.kind().kind(),
            Self::MissingPathParams(_) => ErrorName::MissingPathParams,
            _ => ErrorName::UnknownPathRejectionError,
        }
    }
}

impl ErrorKind for axum::extract::rejection::QueryRejection {
    fn kind(&self) -> ErrorName {
        match self {
            Self::FailedToDeserializeQueryString(_) => ErrorName::FailedToDeserializeQueryString,
            _ => ErrorName::UnknownQueryRejectionError,
        }
    }
}

impl ErrorKind for base64::DecodeError {
    fn kind(&self) -> ErrorName {
        match self {
            Self::InvalidByte(..) => ErrorName::InvalidByte,
            Self::InvalidLastSymbol(..) => ErrorName::InvalidLastSymbol,
            Self::InvalidLength(_) => ErrorName::InvalidLength,
            Self::InvalidPadding => ErrorName::InvalidPadding,
        }
    }
}

impl ErrorKind for crate::auth::header::AuthenticationError {
    fn kind(&self) -> ErrorName {
        match self {
            Self::ExpiredToken => ErrorName::ExpiredToken,
            Self::FailedConnection(_) => ErrorName::FailedConnection,
            Self::FailedQuery(err) => err.kind(),
            Self::InvalidAuthType => ErrorName::InvalidAuthType,
            Self::InvalidEncoding(err) => err.kind(),
            Self::MalformedCredentials => ErrorName::MalformedCredentials,
            Self::MalformedToken(_) => ErrorName::MalformedToken,
            Self::UsernamePasswordMismatch => ErrorName::UsernamePasswordMismatch,
            Self::UsernameTokenMismatch => ErrorName::UsernameTokenMismatch,
            Self::Utf8Conversion(_) => ErrorName::Utf8ConversionError,
        }
    }
}

impl ErrorKind for crate::model::enums::ResourceProperty {
    fn kind(&self) -> ErrorName {
        match self {
            Self::PoolName => ErrorName::PoolNameAlreadyExists,
            Self::PoolPost => ErrorName::PoolPostAlreadyExists,
            Self::PoolCategoryName => ErrorName::PoolCategoryNameAlreadyExists,
            Self::PostContent => ErrorName::DuplicatePost,
            Self::PostFeature => ErrorName::PostAlreadyFeatured,
            Self::PostRelation => ErrorName::PostRelationAlreadyExists,
            Self::TagName => ErrorName::TagNameAlreadyExists,
            Self::TagCategoryName => ErrorName::TagCategoryNameAlreadyExists,
            Self::UserName => ErrorName::UserNameAlreadyExists,
            Self::UserEmail => ErrorName::UserEmailAlreadyExists,
        }
    }
}

impl ErrorKind for crate::model::enums::ResourceType {
    fn kind(&self) -> ErrorName {
        match self {
            Self::Comment => ErrorName::CommentNotFound,
            Self::Pool => ErrorName::PoolNotFound,
            Self::PoolCategory => ErrorName::PoolCategoryNotFound,
            Self::Post => ErrorName::PostNotFound,
            Self::Tag | Self::TagImplication | Self::TagSuggestion => ErrorName::TagNotFound,
            Self::TagCategory => ErrorName::TagCategoryNotFound,
            Self::User => ErrorName::UserNotFound,
            Self::UserToken => ErrorName::UserTokenNotFound,
        }
    }
}

impl ErrorKind for crate::search::TimeParsingError {
    fn kind(&self) -> ErrorName {
        match self {
            Self::TooFewArgs => ErrorName::TooFewArgs,
            Self::TooManyArgs => ErrorName::TooManyArgs,
            Self::NotAnInteger(err) => err.kind().kind(),
            Self::OutOfRange(_) => ErrorName::OutOfRange,
        }
    }
}

impl ErrorKind for diesel::result::DatabaseErrorKind {
    fn kind(&self) -> ErrorName {
        match self {
            Self::CheckViolation => ErrorName::CheckViolation,
            Self::ClosedConnection => ErrorName::ClosedConnection,
            Self::ForeignKeyViolation => ErrorName::ForeignKeyViolation,
            Self::NotNullViolation => ErrorName::NotNullViolation,
            Self::ReadOnlyTransaction => ErrorName::ReadOnlyTransaction,
            Self::SerializationFailure => ErrorName::SerializationFailure,
            Self::UnableToSendCommand => ErrorName::UnableToSendCommand,
            Self::UniqueViolation => ErrorName::UniqueViolation,
            _ => ErrorName::UnknownDatabaseError,
        }
    }
}

impl ErrorKind for diesel::result::Error {
    fn kind(&self) -> ErrorName {
        match self {
            Self::AlreadyInTransaction => ErrorName::AlreadyInTransaction,
            Self::BrokenTransactionManager => ErrorName::BrokenTransactionManager,
            Self::DatabaseError(err, _) => err.kind(),
            Self::DeserializationError(_) => ErrorName::DeserializationError,
            Self::InvalidCString(_) => ErrorName::InvalidCString,
            Self::NotFound => ErrorName::RowNotFound,
            Self::NotInTransaction => ErrorName::NotInTransaction,
            Self::QueryBuilderError(_) => ErrorName::QueryBuilderError,
            Self::RollbackErrorOnCommit { rollback_error, .. } => rollback_error.kind(),
            Self::RollbackTransaction => ErrorName::RollbackTransaction,
            Self::SerializationError(_) => ErrorName::SerializationError,
            _ => ErrorName::UnknownQueryError,
        }
    }
}

impl ErrorKind for diesel::ConnectionError {
    fn kind(&self) -> ErrorName {
        match self {
            Self::BadConnection(_) => ErrorName::BadConnection,
            Self::CouldntSetupConfiguration(err) => err.kind(),
            Self::InvalidCString(_) => ErrorName::InvalidCString,
            Self::InvalidConnectionUrl(_) => ErrorName::InvalidConnectionUrl,
            _ => ErrorName::UnknownDatabaseConnectionError,
        }
    }
}

impl ErrorKind for image::error::LimitErrorKind {
    fn kind(&self) -> ErrorName {
        match self {
            Self::DimensionError => ErrorName::DimensionLimitsExceeded,
            Self::InsufficientMemory => ErrorName::InsufficientMemory,
            Self::Unsupported { .. } => ErrorName::UnsupportedImageDimensions,
            _ => ErrorName::UnknownImageLimitError,
        }
    }
}

impl ErrorKind for image::error::ParameterErrorKind {
    fn kind(&self) -> ErrorName {
        match self {
            Self::DimensionMismatch => ErrorName::DimensionMismatch,
            Self::FailedAlready => ErrorName::FailedAlready,
            Self::Generic(_) => ErrorName::GenericImageError,
            Self::NoMoreData => ErrorName::NoMoreData,
            _ => ErrorName::UnknownImageParameterError,
        }
    }
}

impl ErrorKind for image::error::UnsupportedErrorKind {
    fn kind(&self) -> ErrorName {
        match self {
            Self::Color(_) => ErrorName::UnsupportedColor,
            Self::Format(_) => ErrorName::UnsupportedFormat,
            Self::GenericFeature(_) => ErrorName::UnsupportedFeature,
            _ => ErrorName::UnknownImageUnsupportedError,
        }
    }
}

impl ErrorKind for image::ImageError {
    fn kind(&self) -> ErrorName {
        match self {
            Self::Decoding(_) => ErrorName::FailedDecoding,
            Self::Encoding(_) => ErrorName::FailedEncoding,
            Self::IoError(err) => err.kind().kind(),
            Self::Limits(err) => err.kind().kind(),
            Self::Parameter(err) => err.kind().kind(),
            Self::Unsupported(err) => err.kind().kind(),
        }
    }
}

impl ErrorKind for lettre::address::AddressError {
    fn kind(&self) -> ErrorName {
        match self {
            Self::MissingParts => ErrorName::EmailAddressMissingParts,
            Self::Unbalanced => ErrorName::EmailAddressUnbalanced,
            Self::InvalidUser => ErrorName::EmailAddressInvalidUser,
            Self::InvalidDomain => ErrorName::EmailAddressInvalidDomain,
            Self::InvalidInput => ErrorName::EmailAddressInvalidInput,
            _ => ErrorName::UnknownEmailAddressError,
        }
    }
}

impl ErrorKind for lettre::error::Error {
    fn kind(&self) -> ErrorName {
        match self {
            Self::MissingFrom => ErrorName::EmailMissingForm,
            Self::MissingTo => ErrorName::EmailMissingTo,
            Self::TooManyFrom => ErrorName::EmailTooManyFrom,
            Self::EmailMissingAt => ErrorName::EmailMissingAt,
            Self::EmailMissingLocalPart => ErrorName::EmailMissingLocalPart,
            Self::EmailMissingDomain => ErrorName::EmailMissingDomain,
            Self::CannotParseFilename => ErrorName::EmailCannotParseFilename,
            Self::Io(err) => err.kind().kind(),
            Self::NonAsciiChars => ErrorName::EmailNonAsciiChars,
        }
    }
}

impl ErrorKind for serde_json::error::Category {
    fn kind(&self) -> ErrorName {
        match self {
            Self::Io => ErrorName::JsonIoError,
            Self::Syntax => ErrorName::JsonInvalidSyntax,
            Self::Data => ErrorName::JsonInvalidData,
            Self::Eof => ErrorName::JsonUnexpectedEOF,
        }
    }
}

impl ErrorKind for std::env::VarError {
    fn kind(&self) -> ErrorName {
        match self {
            Self::NotPresent => ErrorName::EnvironmentVariableNotPresent,
            Self::NotUnicode(_) => ErrorName::EnvironmentVariableNotUnicode,
        }
    }
}

impl ErrorKind for std::io::ErrorKind {
    fn kind(&self) -> ErrorName {
        match self {
            Self::NotFound => ErrorName::FileNotFound,
            Self::PermissionDenied => ErrorName::PermissionDenied,
            Self::ConnectionRefused => ErrorName::ConnectionRefused,
            Self::ConnectionReset => ErrorName::ConnectionReset,
            Self::HostUnreachable => ErrorName::HostUnreachable,
            Self::NetworkUnreachable => ErrorName::NetworkUnreachable,
            Self::ConnectionAborted => ErrorName::ConnectionAborted,
            Self::NotConnected => ErrorName::NotConnected,
            Self::AddrInUse => ErrorName::AddressInUse,
            Self::AddrNotAvailable => ErrorName::AddressNotAvailable,
            Self::NetworkDown => ErrorName::NetworkDown,
            Self::BrokenPipe => ErrorName::BrokenPipe,
            Self::AlreadyExists => ErrorName::FileAlreadyExists,
            Self::WouldBlock => ErrorName::WouldBlock,
            Self::NotADirectory => ErrorName::NotADirectory,
            Self::IsADirectory => ErrorName::IsADirectory,
            Self::DirectoryNotEmpty => ErrorName::DirectoryNotEmpty,
            Self::ReadOnlyFilesystem => ErrorName::ReadOnlyFilesystem,
            Self::StaleNetworkFileHandle => ErrorName::StaleNetworkFileHandle,
            Self::InvalidInput => ErrorName::InvalidInput,
            Self::InvalidData => ErrorName::InvalidData,
            Self::TimedOut => ErrorName::TimedOut,
            Self::WriteZero => ErrorName::WriteZero,
            Self::StorageFull => ErrorName::StorageFull,
            Self::NotSeekable => ErrorName::NotSeekable,
            Self::QuotaExceeded => ErrorName::QuotaExceeded,
            Self::FileTooLarge => ErrorName::FileTooLarge,
            Self::ResourceBusy => ErrorName::ResourceBusy,
            Self::ExecutableFileBusy => ErrorName::ExecutableFileBusy,
            Self::Deadlock => ErrorName::Deadlock,
            Self::CrossesDevices => ErrorName::CrossesDevices,
            Self::TooManyLinks => ErrorName::TooManyLinks,
            Self::InvalidFilename => ErrorName::InvalidFilename,
            Self::ArgumentListTooLong => ErrorName::ArgumentListTooLong,
            Self::Interrupted => ErrorName::Interrupted,
            Self::Unsupported => ErrorName::Unsupported,
            Self::UnexpectedEof => ErrorName::UnexpectedEof,
            Self::OutOfMemory => ErrorName::OutOfMemory,
            Self::Other => ErrorName::OtherIoError,
            _ => ErrorName::UnknownIoError,
        }
    }
}

impl ErrorKind for std::num::IntErrorKind {
    fn kind(&self) -> ErrorName {
        match self {
            Self::Empty => ErrorName::EmptyValue,
            Self::InvalidDigit => ErrorName::InvalidDigit,
            Self::PosOverflow => ErrorName::PositiveOverflow,
            Self::NegOverflow => ErrorName::NegativeOverflow,
            Self::Zero => ErrorName::ZeroNotAllowed,
            _ => ErrorName::UnknownIntParseError,
        }
    }
}

impl ErrorKind for swf::error::Error {
    fn kind(&self) -> ErrorName {
        match self {
            Self::Avm1ParseError { .. } => ErrorName::SwfAvm1ParseError,
            Self::InvalidData(_) => ErrorName::SwfInvalidData,
            Self::SwfParseError { .. } => ErrorName::SwfParseError,
            Self::IoError(_) => ErrorName::SwfIoError,
            Self::Unsupported(_) => ErrorName::SwfUnsupported,
        }
    }
}

impl ErrorKind for video_rs::ffmpeg::Error {
    fn kind(&self) -> ErrorName {
        match self {
            Self::Bug => ErrorName::FFmpegBug1,
            Self::Bug2 => ErrorName::FFmpegBug2,
            Self::Unknown => ErrorName::FFmpegUnknown,
            Self::Experimental => ErrorName::FFmpegExperimental,
            Self::BufferTooSmall => ErrorName::FFmpegBufferTooSmall,
            Self::Eof => ErrorName::FFmpegEof,
            Self::Exit => ErrorName::FFmpegExit,
            Self::External => ErrorName::FFmpegExternal,
            Self::InvalidData => ErrorName::FFmpegInvalidData,
            Self::PatchWelcome => ErrorName::FFmpegPatchWelcome,
            Self::InputChanged => ErrorName::FFmpegInputChanged,
            Self::OutputChanged => ErrorName::FFmpegOutputChanged,
            Self::BsfNotFound => ErrorName::FFmpegBsfNotFound,
            Self::DecoderNotFound => ErrorName::FFmpegDecoderNotFound,
            Self::DemuxerNotFound => ErrorName::FFmpegDemuxerNotFound,
            Self::EncoderNotFound => ErrorName::FFmpegEncoderNotFound,
            Self::OptionNotFound => ErrorName::FFmpegOptionNotFound,
            Self::MuxerNotFound => ErrorName::FFmpegMuxerNotFound,
            Self::FilterNotFound => ErrorName::FFmpegFilterNotFound,
            Self::ProtocolNotFound => ErrorName::FFmpegProtocolNotFound,
            Self::StreamNotFound => ErrorName::FFmpegStreamNotFound,
            Self::HttpBadRequest => ErrorName::FFmpegHttpBadRequest,
            Self::HttpUnauthorized => ErrorName::FFmpegHttpUnauthorized,
            Self::HttpForbidden => ErrorName::FFmpegHttpForbidden,
            Self::HttpNotFound => ErrorName::FFmpegHttpNotFound,
            Self::HttpOther4xx => ErrorName::FFmpegHttpOther4xx,
            Self::HttpServerError => ErrorName::FFmpegHttpServerError,
            Self::Other { .. } => ErrorName::FFmpegPosixError,
        }
    }
}

impl ErrorKind for video_rs::Error {
    fn kind(&self) -> ErrorName {
        match self {
            Self::ReadExhausted => ErrorName::ReadExhausted,
            Self::DecodeExhausted => ErrorName::DecodeExhausted,
            Self::WriteRetryLimitReached => ErrorName::WriteRetryLimitReached,
            Self::InvalidFrameFormat => ErrorName::InvalidFrameFormat,
            Self::InvalidExtraData => ErrorName::InvalidExtraData,
            Self::MissingCodecParameters => ErrorName::MissingCodecParameters,
            Self::UnsupportedCodecParameterSets => ErrorName::UnsupportedCodecParameterSets,
            Self::InvalidResizeParameters => ErrorName::InvalidResizeParameters,
            Self::UninitializedCodec => ErrorName::UninitializedCodec,
            Self::UnsupportedCodecHardwareAccelerationDeviceType => {
                ErrorName::UnsupportedCodecHardwareAccelerationDeviceType
            }
            Self::BackendError(err) => err.kind(),
        }
    }
}

impl ErrorKind for crate::api::error::ApiError {
    fn kind(&self) -> ErrorName {
        match self {
            Self::AlreadyExists(err) => err.kind(),
            Self::ContentTypeMismatch(..) => ErrorName::ContentTypeMismatch,
            Self::CyclicDependency(_) => ErrorName::CyclicDependency,
            Self::DeleteDefault(_) => ErrorName::DeleteDefault,
            Self::EmptySwf => ErrorName::EmptySwf,
            Self::EmptyVideo => ErrorName::EmptyVideo,
            Self::ExpressionFailsRegex(..) => ErrorName::ExpressionFailsRegex,
            Self::FailedAuthentication(err) => err.kind(),
            Self::FailedConnection(_) => ErrorName::FailedConnection,
            Self::FailedEmailTransport(_) => ErrorName::FailedEmailTransport,
            Self::FailedQuery(err) => err.kind(),
            Self::FromStr(_) => ErrorName::FromStrError,
            Self::HeaderDeserialization(_) => ErrorName::HeaderDeserialization,
            Self::Hidden(_) => ErrorName::ResourceHidden,
            Self::InsufficientPrivileges => ErrorName::InsufficientPrivileges,
            Self::InvalidEmailAddress(err) => err.kind(),
            Self::InvalidEmail(err) => err.kind(),
            Self::InvalidHeader(_) => ErrorName::InvalidHeader,
            Self::InvalidSort => ErrorName::InvalidSort,
            Self::InvalidTime(err) => err.kind(),
            Self::InvalidUploadToken => ErrorName::InvalidUploadToken,
            Self::InvalidUserRank => ErrorName::InvalidUserRank,
            Self::Image(err) => err.kind(),
            Self::JsonRejection(err) => err.kind(),
            Self::JsonSerialization(err) => err.classify().kind(),
            Self::NoEmail => ErrorName::NoEmail,
            Self::MissingContent(_) => ErrorName::MissingContent,
            Self::MissingContentType => ErrorName::MissingContentType,
            Self::MissingFormData => ErrorName::MissingFormData,
            Self::MissingMetadata => ErrorName::MissingMetadata,
            Self::MissingSmtpInfo => ErrorName::MissingSmtpInfo,
            Self::Multipart(_) => ErrorName::MultipartError,
            Self::MultipartRejection(err) => err.kind(),
            Self::NoNamesGiven(_) => ErrorName::NoNamesGiven,
            Self::NotAnInteger(err) => err.kind().kind(),
            Self::NotFound(err) => err.kind(),
            Self::NotLoggedIn => ErrorName::NotLoggedIn,
            Self::Password(err) => err.kind(),
            Self::PathRejection(err) => err.kind(),
            Self::QueryRejection(err) => err.kind(),
            Self::Request(_) => ErrorName::RequestError,
            Self::ResourceModified => ErrorName::ResourceModified,
            Self::SelfMerge(_) => ErrorName::SelfMerge,
            Self::StdIo(err) => err.kind().kind(),
            Self::SwfDecoding(err) => err.kind(),
            Self::UnimplementedFrameFormat(_) => ErrorName::UnimplementedFrameFormat,
            Self::UnauthorizedPasswordReset => ErrorName::UnauthorizedPasswordReset,
            Self::UnsupportedExtension(_) => ErrorName::UnsupportedExtension,
            Self::VideoDecoding(err) => err.kind(),
        }
    }
}

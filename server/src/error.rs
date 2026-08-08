use serde::Serialize;
use utoipa::ToSchema;

#[derive(Serialize, ToSchema)]
pub enum ErrorName {
    AddressInUse,
    AddressNotAvailable,
    AlphabetTooLargeHuff,
    AlreadyInTransaction,
    AnsChecksumMismatch,
    ArgumentListTooLong,
    ArithmeticOverflow,
    BadConnection,
    BaseColorCorrelationOutOfRange,
    BlockContextMapSizeTooBig,
    BrokenPipe,
    BrokenTransactionManager,
    BytesRejection,
    CheckViolation,
    ClosedConnection,
    CommentNotFound,
    ConnectionAborted,
    ConnectionRefused,
    ConnectionReset,
    CopyOfDifferentSize,
    CrossesDevices,
    CryptoError,
    CyclicDependency,
    Deadlock,
    DeleteDefault,
    DeserializationError,
    DimensionLimitsExceeded,
    DimensionMismatch,
    DimShiftTooLarge,
    DirectoryNotEmpty,
    DisabledToken,
    DownloadTooLarge,
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
    EndOfBlockResidualNonZeros,
    ExecutableFileBusy,
    ExpiredToken,
    ExpressionFailsRegex,
    ExtensionRejection,
    FailedAlready,
    FailedConnection,
    FailedDecoding,
    FailedEmailTransport,
    FailedEncoding,
    FailedToDeserializeQueryString,
    FFmpegError,
    FileAlreadyExists,
    FileNotFound,
    FileTooLarge,
    FloatNaNOrInf,
    ForeignKeyViolation,
    FrameBufferMismatch,
    FromStrError,
    GenericImageError,
    HeaderDeserialization,
    HFBlockOutOfBounds,
    HfQuantFactorTooSmall,
    HostUnreachable,
    IccEndOfStream,
    IccInvalidTagString,
    IccInvalidWhitePoint,
    IccInvalidWhitePointY,
    IccMlucTextNotAscii,
    IccTableSizeExceeded,
    IccTooLarge,
    IccUnsupportedTransferFunction,
    IccValueOutOfRangeS15Fixed16,
    IccWriteOutOfBounds,
    ImageDimensionTooLarge,
    ImageOutOfMemory,
    ImageSizeTooLarge,
    InsufficientMemory,
    InsufficientPrivileges,
    IntegerTooLarge,
    Interrupted,
    InvalidAFVBands,
    InvalidAnsHistogram,
    InvalidAuthType,
    InvalidBitsPerSample,
    InvalidBlendingAlphaChannel,
    InvalidBlockSizeForChromaSubsampling,
    InvalidBoundary,
    InvalidBox,
    InvalidByte,
    InvalidChannelRange,
    InvalidCharacter,
    InvalidColorEncoding,
    InvalidColorSpace,
    InvalidConnectionUrl,
    InvalidContextMap,
    InvalidContextMapHole,
    InvalidCString,
    InvalidData,
    InvalidDigit,
    InvalidDistanceBand,
    InvalidEcUpsampling,
    InvalidEncoding,
    InvalidEnum,
    InvalidEpfValue,
    InvalidExponent,
    InvalidFilename,
    InvalidFormat,
    InvalidGamma,
    InvalidHeader,
    InvalidHistogramIndex,
    InvalidHuffman,
    InvalidIccStream,
    InvalidImageSize,
    InvalidInput,
    InvalidIntensityTarget,
    InvalidLastSymbol,
    InvalidLength,
    InvalidLfLevel,
    InvalidLinearBelow,
    InvalidMantissa,
    InvalidMime,
    InvalidMinNits,
    InvalidNumNonZeros,
    InvalidOutputBufferSize,
    InvalidPadding,
    InvalidPassword,
    InvalidPermutationLehmerCode,
    InvalidPermutationSize,
    InvalidPhcStringField,
    InvalidPredictor,
    InvalidProperty,
    InvalidQuantEncoding,
    InvalidQuantEncodingMode,
    InvalidQuantizationTableWeight,
    InvalidRawQuantTable,
    InvalidRCT,
    InvalidRenderingIntent,
    InvalidSignature,
    InvalidSort,
    InvalidTransformId,
    InvalidUintConfig,
    InvalidUploadToken,
    InvalidUserRank,
    InvalidUtf8InPathParam,
    InvalidVarDCTTransform,
    InvalidVarDCTTransformMap,
    InvalidVersion,
    IsADirectory,
    JpegXlOutOfMemory,
    JsonDataError,
    JsonInvalidData,
    JsonInvalidSyntax,
    JsonIoError,
    JsonSyntaxError,
    JsonUnexpectedEOF,
    LfQuantFactorTooSmall,
    Lz77Disallowed,
    MalformedCredentials,
    MalformedToken,
    MalformedValue,
    MatrixInversionFailed,
    MetaSqueezeRequiresInPlace,
    MissingContent,
    MissingContentType,
    MissingFormData,
    MissingJsonContentType,
    MissingMetadata,
    MissingPathParams,
    MissingSmtpInfo,
    MixingDifferentChannels,
    MultipartError,
    NegativeOverflow,
    NetworkDown,
    NetworkUnreachable,
    NoEmail,
    NoGlobalTree,
    NoLfFrame,
    NoMoreData,
    Non444ChromaSubsampling,
    NoNamesGiven,
    NonPatchReferenceWithCrop,
    NonZeroPadding,
    NotADirectory,
    NotConnected,
    NotGrayscale,
    NotInTransaction,
    NotLoggedIn,
    NotNullViolation,
    NotSeekable,
    NumPassesTooLarge,
    OtherIoError,
    OtherPathError,
    OutOfBounds,
    OutOfMemory,
    OutOfRange,
    ParamNameDuplicated,
    ParamNameInvalid,
    ParamsMaxExceeded,
    PassesDownsampleNonDecreasing,
    PassesLastPassNonIncreasing,
    PassesLastPassTooLarge,
    PatchesInvalidAlphaChannel,
    PatchesInvalidBlendMode,
    PatchesInvalidDelta,
    PatchesInvalidPosition,
    PatchesInvalidReference,
    PatchesOutOfBounds,
    PatchesPostColorTransform,
    PatchesRefTooLarge,
    PatchesTooMany,
    PatchesUnsupportedMixedUpsampling,
    PathDeserializeError,
    PathParseError,
    PathParseErrorAtIndex,
    PathParseErrorAtKey,
    PermissionDenied,
    PhcStringTrailingData,
    PipelineChannelTypeMismatch,
    PipelineInvalidStageAfterExtend,
    PointListEmpty,
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
    ReadOnlyFilesystem,
    ReadOnlyTransaction,
    RequestError,
    ResourceBusy,
    ResourceHidden,
    ResourceModified,
    RollbackTransaction,
    RowNotFound,
    SaveDifferentDownsample,
    SectionTooShort,
    SelfMerge,
    SerializationError,
    SerializationFailure,
    SizeOverflow,
    SplineAdjacentCoincidingControlPoints,
    SplinesAreaTooLarge,
    SplinesCoordinatesLimit,
    SplinesDeltaLimit,
    SplinesDistanceTooLarge,
    SplinesPointOutOfRange,
    SplinesTooMany,
    SplinesTooManyControlPoints,
    StaleNetworkFileHandle,
    StorageFull,
    SwfAvm1ParseError,
    SwfInvalidData,
    SwfParseError,
    SwfUnsupported,
    TagCategoryNameAlreadyExists,
    TagCategoryNotFound,
    TagNameAlreadyExists,
    TagNotFound,
    TaskCancelled,
    TaskPanicked,
    TimedOut,
    TooManyArgs,
    TooManyBlockContexts,
    TooManyExtraChannels,
    TooManyLinks,
    TooManySqueezes,
    TransferFunctionUnknown,
    TreeMultiplierBitsTooLarge,
    TreeMultiplierTooLarge,
    TreeSplitOnEmptyRange,
    TreeTooLarge,
    TreeTooTall,
    UnableToSendCommand,
    UnauthorizedPasswordReset,
    UnexpectedEof,
    UnexpectedLz77Repeat,
    UnexpectedOutputSize,
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
    UnknownJpegXlError,
    UnknownJsonRejectionError,
    UnknownMultipartRejectionError,
    UnknownPathDeserializeError,
    UnknownPathRejectionError,
    UnknownQueryError,
    UnknownQueryRejectionError,
    Unsupported,
    UnsupportedAlgorithm,
    UnsupportedColor,
    UnsupportedContentType,
    UnsupportedExtension,
    UnsupportedFeature,
    UnsupportedFormat,
    UnsupportedImageDimensions,
    UnsupportedPathType,
    UrlValidationError,
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
    WriteZero,
    WrongBufferCount,
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
            Self::DisabledToken => ErrorName::DisabledToken,
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

impl ErrorKind for jxl::error::Error {
    fn kind(&self) -> ErrorName {
        match self {
            Self::InvalidRawQuantTable => ErrorName::InvalidRawQuantTable,
            Self::InvalidDistanceBand(..) => ErrorName::InvalidDistanceBand,
            Self::InvalidAFVBands => ErrorName::InvalidAFVBands,
            Self::InvalidQuantizationTableWeight(_) => ErrorName::InvalidQuantizationTableWeight,
            Self::OutOfBounds(_) => ErrorName::OutOfBounds,
            Self::SectionTooShort => ErrorName::SectionTooShort,
            Self::NonZeroPadding => ErrorName::NonZeroPadding,
            Self::InvalidSignature => ErrorName::InvalidSignature,
            Self::InvalidExponent(_) => ErrorName::InvalidExponent,
            Self::InvalidMantissa(_) => ErrorName::InvalidMantissa,
            Self::InvalidBitsPerSample(_) => ErrorName::InvalidBitsPerSample,
            Self::InvalidEnum(..) => ErrorName::InvalidEnum,
            Self::DimShiftTooLarge(_) => ErrorName::DimShiftTooLarge,
            Self::FloatNaNOrInf => ErrorName::FloatNaNOrInf,
            Self::InvalidGamma(_) => ErrorName::InvalidGamma,
            Self::InvalidColorEncoding => ErrorName::InvalidColorEncoding,
            Self::InvalidColorSpace => ErrorName::InvalidColorSpace,
            Self::InvalidRenderingIntent => ErrorName::InvalidRenderingIntent,
            Self::InvalidIntensityTarget(_) => ErrorName::InvalidIntensityTarget,
            Self::InvalidMinNits(_) => ErrorName::InvalidMinNits,
            Self::InvalidLinearBelow(..) => ErrorName::InvalidLinearBelow,
            Self::SizeOverflow => ErrorName::SizeOverflow,
            Self::InvalidBox => ErrorName::InvalidBox,
            Self::IccTooLarge => ErrorName::IccTooLarge,
            Self::IccEndOfStream => ErrorName::IccEndOfStream,
            Self::InvalidIccStream => ErrorName::InvalidIccStream,
            Self::InvalidUintConfig(..) => ErrorName::InvalidUintConfig,
            Self::Lz77Disallowed => ErrorName::Lz77Disallowed,
            Self::UnexpectedLz77Repeat => ErrorName::UnexpectedLz77Repeat,
            Self::AlphabetTooLargeHuff(_) => ErrorName::AlphabetTooLargeHuff,
            Self::InvalidHuffman => ErrorName::InvalidHuffman,
            Self::InvalidAnsHistogram => ErrorName::InvalidAnsHistogram,
            Self::AnsChecksumMismatch => ErrorName::AnsChecksumMismatch,
            Self::IntegerTooLarge(_) => ErrorName::IntegerTooLarge,
            Self::InvalidContextMap(_) => ErrorName::InvalidContextMap,
            Self::InvalidContextMapHole(..) => ErrorName::InvalidContextMapHole,
            Self::InvalidPermutationSize { .. } => ErrorName::InvalidPermutationSize,
            Self::InvalidPermutationLehmerCode { .. } => ErrorName::InvalidPermutationLehmerCode,
            Self::InvalidQuantEncodingMode => ErrorName::InvalidQuantEncodingMode,
            Self::InvalidQuantEncoding { .. } => ErrorName::InvalidQuantEncoding,
            Self::InvalidEcUpsampling(..) => ErrorName::InvalidEcUpsampling,
            Self::InvalidLfLevel(_) => ErrorName::InvalidLfLevel,
            Self::NumPassesTooLarge(..) => ErrorName::NumPassesTooLarge,
            Self::PassesDownsampleNonDecreasing => ErrorName::PassesDownsampleNonDecreasing,
            Self::PassesLastPassNonIncreasing => ErrorName::PassesLastPassNonIncreasing,
            Self::PassesLastPassTooLarge => ErrorName::PassesLastPassTooLarge,
            Self::NonPatchReferenceWithCrop => ErrorName::NonPatchReferenceWithCrop,
            Self::Non444ChromaSubsampling => ErrorName::Non444ChromaSubsampling,
            Self::InvalidBlockSizeForChromaSubsampling => ErrorName::InvalidBlockSizeForChromaSubsampling,
            Self::OutOfMemory(_) => ErrorName::JpegXlOutOfMemory,
            Self::ImageOutOfMemory(..) => ErrorName::ImageOutOfMemory,
            Self::ImageSizeTooLarge(..) => ErrorName::ImageSizeTooLarge,
            Self::ImageDimensionTooLarge(_) => ErrorName::ImageDimensionTooLarge,
            Self::InvalidImageSize(..) => ErrorName::InvalidImageSize,
            Self::ArithmeticOverflow => ErrorName::ArithmeticOverflow,
            Self::PipelineChannelTypeMismatch(..) => ErrorName::PipelineChannelTypeMismatch,
            Self::PipelineInvalidStageAfterExtend(_) => ErrorName::PipelineInvalidStageAfterExtend,
            Self::CopyOfDifferentSize(..) => ErrorName::CopyOfDifferentSize,
            Self::LfQuantFactorTooSmall(_) => ErrorName::LfQuantFactorTooSmall,
            Self::HfQuantFactorTooSmall(_) => ErrorName::HfQuantFactorTooSmall,
            Self::InvalidPredictor(_) => ErrorName::InvalidPredictor,
            Self::InvalidProperty(_) => ErrorName::InvalidProperty,
            Self::InvalidBlendingAlphaChannel(..) => ErrorName::InvalidBlendingAlphaChannel,
            Self::PatchesInvalidAlphaChannel(..) => ErrorName::PatchesInvalidAlphaChannel,
            Self::PatchesInvalidBlendMode(..) => ErrorName::PatchesInvalidBlendMode,
            Self::PatchesInvalidDelta(..) => ErrorName::PatchesInvalidDelta,
            Self::PatchesInvalidPosition(..) => ErrorName::PatchesInvalidPosition,
            Self::PatchesInvalidReference(_) => ErrorName::PatchesInvalidReference,
            Self::PatchesOutOfBounds(..) => ErrorName::PatchesOutOfBounds,
            Self::PatchesPostColorTransform() => ErrorName::PatchesPostColorTransform,
            Self::PatchesUnsupportedMixedUpsampling(..) => ErrorName::PatchesUnsupportedMixedUpsampling,
            Self::PatchesTooMany(..) => ErrorName::PatchesTooMany,
            Self::PatchesRefTooLarge(..) => ErrorName::PatchesRefTooLarge,
            Self::PointListEmpty => ErrorName::PointListEmpty,
            Self::SplinesAreaTooLarge(..) => ErrorName::SplinesAreaTooLarge,
            Self::SplinesDistanceTooLarge(..) => ErrorName::SplinesDistanceTooLarge,
            Self::SplinesTooMany(..) => ErrorName::SplinesTooMany,
            Self::SplineAdjacentCoincidingControlPoints(..) => ErrorName::SplineAdjacentCoincidingControlPoints,
            Self::SplinesTooManyControlPoints(..) => ErrorName::SplinesTooManyControlPoints,
            Self::SplinesPointOutOfRange(..) => ErrorName::SplinesPointOutOfRange,
            Self::SplinesCoordinatesLimit(..) => ErrorName::SplinesCoordinatesLimit,
            Self::SplinesDeltaLimit(..) => ErrorName::SplinesDeltaLimit,
            Self::TreeTooLarge(..) => ErrorName::TreeTooLarge,
            Self::TreeTooTall(..) => ErrorName::TreeTooTall,
            Self::TreeMultiplierTooLarge(..) => ErrorName::TreeMultiplierTooLarge,
            Self::TreeMultiplierBitsTooLarge(..) => ErrorName::TreeMultiplierBitsTooLarge,
            Self::TreeSplitOnEmptyRange(..) => ErrorName::TreeSplitOnEmptyRange,
            Self::NoGlobalTree => ErrorName::NoGlobalTree,
            Self::InvalidTransformId => ErrorName::InvalidTransformId,
            Self::InvalidRCT(_) => ErrorName::InvalidRCT,
            Self::InvalidChannelRange(..) => ErrorName::InvalidChannelRange,
            Self::MixingDifferentChannels => ErrorName::MixingDifferentChannels,
            Self::MetaSqueezeRequiresInPlace => ErrorName::MetaSqueezeRequiresInPlace,
            Self::TooManySqueezes => ErrorName::TooManySqueezes,
            Self::BlockContextMapSizeTooBig(..) => ErrorName::BlockContextMapSizeTooBig,
            Self::TooManyBlockContexts => ErrorName::TooManyBlockContexts,
            Self::BaseColorCorrelationOutOfRange => ErrorName::BaseColorCorrelationOutOfRange,
            Self::InvalidEpfValue(_) => ErrorName::InvalidEpfValue,
            Self::InvalidVarDCTTransform(_) => ErrorName::InvalidVarDCTTransform,
            Self::InvalidVarDCTTransformMap => ErrorName::InvalidVarDCTTransformMap,
            Self::HFBlockOutOfBounds => ErrorName::HFBlockOutOfBounds,
            Self::InvalidNumNonZeros(..) => ErrorName::InvalidNumNonZeros,
            Self::InvalidHistogramIndex(..) => ErrorName::InvalidHistogramIndex,
            Self::EndOfBlockResidualNonZeros(_) => ErrorName::EndOfBlockResidualNonZeros,
            Self::TransferFunctionUnknown => ErrorName::TransferFunctionUnknown,
            Self::IccWriteOutOfBounds => ErrorName::IccWriteOutOfBounds,
            Self::IccInvalidTagString(_) => ErrorName::IccInvalidTagString,
            Self::IccMlucTextNotAscii(_) => ErrorName::IccMlucTextNotAscii,
            Self::IccValueOutOfRangeS15Fixed16(_) => ErrorName::IccValueOutOfRangeS15Fixed16,
            Self::IccInvalidWhitePointY(_) => ErrorName::IccInvalidWhitePointY,
            Self::IccInvalidWhitePoint(..) => ErrorName::IccInvalidWhitePoint,
            Self::MatrixInversionFailed(_) => ErrorName::MatrixInversionFailed,
            Self::IccUnsupportedTransferFunction => ErrorName::IccUnsupportedTransferFunction,
            Self::IccTableSizeExceeded(_) => ErrorName::IccTableSizeExceeded,
            Self::IOError(err) => err.kind().kind(),
            Self::WrongBufferCount(..) => ErrorName::WrongBufferCount,
            Self::NotGrayscale => ErrorName::NotGrayscale,
            Self::InvalidOutputBufferSize(..) => ErrorName::InvalidOutputBufferSize,
            Self::SaveDifferentDownsample(..) => ErrorName::SaveDifferentDownsample,
            Self::TooManyExtraChannels(_) => ErrorName::TooManyExtraChannels,
            Self::NoLfFrame(_) => ErrorName::NoLfFrame,
            _ => ErrorName::UnknownJpegXlError,
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
            Self::IoError(err) => err.kind().kind(),
            Self::Unsupported(_) => ErrorName::SwfUnsupported,
        }
    }
}

impl ErrorKind for tokio::task::JoinError {
    fn kind(&self) -> ErrorName {
        if self.is_panic() {
            ErrorName::TaskPanicked
        } else {
            ErrorName::TaskCancelled
        }
    }
}

impl ErrorKind for crate::api::error::ApiError {
    fn kind(&self) -> ErrorName {
        match self {
            Self::AlreadyExists(err) => err.kind(),
            Self::CyclicDependency(_) => ErrorName::CyclicDependency,
            Self::DeleteDefault(_) => ErrorName::DeleteDefault,
            Self::DownloadTooLarge => ErrorName::DownloadTooLarge,
            Self::EmptySwf => ErrorName::EmptySwf,
            Self::EmptyVideo => ErrorName::EmptyVideo,
            Self::ExpressionFailsRegex(..) => ErrorName::ExpressionFailsRegex,
            Self::ExtensionRejection(_) => ErrorName::ExtensionRejection,
            Self::FailedAuthentication(err) => err.kind(),
            Self::FailedConnection(_) => ErrorName::FailedConnection,
            Self::FailedEmailTransport(_) => ErrorName::FailedEmailTransport,
            Self::FailedQuery(err) => err.kind(),
            Self::FfmpegError(_) => ErrorName::FFmpegError,
            Self::FrameBufferMismatch(..) => ErrorName::FrameBufferMismatch,
            Self::FromStr(_) => ErrorName::FromStrError,
            Self::HeaderDeserialization(_) => ErrorName::HeaderDeserialization,
            Self::Hidden(_) => ErrorName::ResourceHidden,
            Self::InsufficientPrivileges => ErrorName::InsufficientPrivileges,
            Self::InvalidEmailAddress(err) => err.kind(),
            Self::InvalidEmail(err) => err.kind(),
            Self::InvalidHeader(_) => ErrorName::InvalidHeader,
            Self::InvalidMime(_) => ErrorName::InvalidMime,
            Self::InvalidSort => ErrorName::InvalidSort,
            Self::InvalidTime(err) => err.kind(),
            Self::InvalidUploadToken => ErrorName::InvalidUploadToken,
            Self::InvalidUserRank => ErrorName::InvalidUserRank,
            Self::Image(err) => err.kind(),
            Self::JsonRejection(err) => err.kind(),
            Self::JsonSerialization(err) => err.classify().kind(),
            Self::JxlDecoding(err) => err.kind(),
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
            Self::TaskJoin(err) => err.kind(),
            Self::UnauthorizedPasswordReset => ErrorName::UnauthorizedPasswordReset,
            Self::UnsupportedContentType(_) => ErrorName::UnsupportedContentType,
            Self::UnsupportedExtension(_) => ErrorName::UnsupportedExtension,
            Self::UrlValidation(_) => ErrorName::UrlValidationError,
        }
    }
}

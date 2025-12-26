use crate::auth::header::AuthenticationError;
use crate::config::RegexType;
use crate::error::ErrorKind;
use crate::model::enums::{MimeType, ResourceProperty, ResourceType};
use crate::string::SmallString;
use axum::Json;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use diesel::QueryResult;
use image::error::{ImageError, LimitError, LimitErrorKind};
use serde::Serialize;

pub type ApiResult<T> = Result<T, ApiError>;

/// Giant error enum of doom
#[derive(Debug, thiserror::Error)]
#[error(transparent)]
pub enum ApiError {
    #[error("{0} already exists")]
    AlreadyExists(ResourceProperty),
    #[error("File of type {0} did not match request with content-type '{1}'")]
    ContentTypeMismatch(MimeType, SmallString),
    #[error("Cyclic dependency detected in {0}s")]
    CyclicDependency(ResourceType),
    #[error("Cannot delete default {0}")]
    DeleteDefault(ResourceType),
    #[error("SWF has no decodable images")]
    EmptySwf,
    #[error("Video file has no frames")]
    EmptyVideo,
    #[error("'{0}' does not match on {1} regex")]
    ExpressionFailsRegex(SmallString, RegexType),
    FailedAuthentication(#[from] AuthenticationError),
    FailedConnection(#[from] diesel::r2d2::PoolError),
    FailedEmailTransport(#[from] lettre::transport::smtp::Error),
    FailedQuery(#[from] diesel::result::Error),
    FromStr(#[from] Box<dyn std::error::Error + Send + Sync>),
    HeaderDeserialization(#[from] axum::http::header::ToStrError),
    #[error("{0} hidden")]
    Hidden(ResourceType),
    #[error("Insufficient privileges")]
    InsufficientPrivileges,
    InvalidEmailAddress(#[from] lettre::address::AddressError),
    InvalidEmail(#[from] lettre::error::Error),
    InvalidHeader(#[from] reqwest::header::InvalidHeaderValue),
    #[error("Invalid sort token")]
    InvalidSort,
    InvalidTime(#[from] crate::search::TimeParsingError),
    #[error("Cannot create an anonymous user")]
    InvalidUserRank,
    Image(#[from] image::ImageError),
    JsonRejection(#[from] axum::extract::rejection::JsonRejection),
    JsonSerialization(#[from] serde_json::Error),
    #[error("Missing {0} content")]
    MissingContent(ResourceType),
    #[error("Form is missing content-type")]
    MissingContentType,
    #[error("Missing form data")]
    MissingFormData,
    #[error("Missing metadata form")]
    MissingMetadata,
    #[error("Missing smtp info")]
    MissingSmtpInfo,
    Multipart(#[from] axum::extract::multipart::MultipartError),
    MultipartRejection(#[from] axum::extract::multipart::MultipartRejection),
    #[error("User has no email")]
    NoEmail,
    #[error("{0} needs at least one name")]
    NoNamesGiven(ResourceType),
    NotAnInteger(#[from] std::num::ParseIntError),
    #[error("{0} not found")]
    NotFound(ResourceType),
    #[error("This action requires you to be logged in")]
    NotLoggedIn,
    Password(#[from] argon2::password_hash::Error),
    PathRejection(#[from] axum::extract::rejection::PathRejection),
    QueryRejection(#[from] axum::extract::rejection::QueryRejection),
    Request(#[from] reqwest::Error),
    #[error("Someone else modified this in the meantime. Please try again.")]
    ResourceModified,
    #[error("Cannot merge {0} with itself")]
    SelfMerge(ResourceType),
    StdIo(#[from] std::io::Error),
    SwfDecoding(#[from] swf::error::Error),
    #[error("Frame format {0:?} is unimplemented")]
    UnimplementedFrameFormat(video_rs::ffmpeg::format::Pixel),
    #[error("Password reset token is invalid")]
    UnauthorizedPasswordReset,
    UnsupportedExtension(#[from] crate::model::enums::ParseExtensionError),
    VideoDecoding(#[from] video_rs::Error),
}

impl ApiError {
    fn status_code(&self) -> StatusCode {
        use serde_json::error::Category;
        type QueryError = diesel::result::Error;

        let query_error_status_code = |err: &QueryError| match err {
            QueryError::NotFound => StatusCode::NOT_FOUND,
            _ => StatusCode::INTERNAL_SERVER_ERROR,
        };

        match self {
            Self::JsonRejection(err) => err.status(),
            Self::Multipart(err) => err.status(),
            Self::MultipartRejection(err) => err.status(),
            Self::PathRejection(err) => err.status(),
            Self::QueryRejection(err) => err.status(),
            Self::AlreadyExists(_)
            | Self::ContentTypeMismatch(..)
            | Self::CyclicDependency(_)
            | Self::DeleteDefault(_)
            | Self::EmptySwf
            | Self::EmptyVideo
            | Self::ExpressionFailsRegex(..)
            | Self::FromStr(_)
            | Self::HeaderDeserialization(_)
            | Self::InvalidEmail(_)
            | Self::InvalidEmailAddress(_)
            | Self::InvalidSort
            | Self::InvalidTime(_)
            | Self::InvalidUserRank
            | Self::MissingContent(_)
            | Self::MissingContentType
            | Self::MissingFormData
            | Self::MissingMetadata
            | Self::NoEmail
            | Self::NoNamesGiven(_)
            | Self::NotAnInteger(_)
            | Self::Request(_)
            | Self::SelfMerge(_) => StatusCode::BAD_REQUEST,
            Self::NotLoggedIn | Self::Password(_) | Self::UnauthorizedPasswordReset => StatusCode::UNAUTHORIZED,
            Self::InsufficientPrivileges => StatusCode::FORBIDDEN,
            Self::Hidden(_) | Self::NotFound(_) => StatusCode::NOT_FOUND,
            Self::ResourceModified => StatusCode::CONFLICT,
            Self::UnsupportedExtension(_) => StatusCode::UNSUPPORTED_MEDIA_TYPE,
            Self::FailedEmailTransport(_)
            | Self::InvalidHeader(_)
            | Self::Image(_)
            | Self::MissingSmtpInfo
            | Self::StdIo(_)
            | Self::SwfDecoding(_)
            | Self::VideoDecoding(_) => StatusCode::INTERNAL_SERVER_ERROR,
            Self::UnimplementedFrameFormat(_) => StatusCode::NOT_IMPLEMENTED,
            Self::FailedConnection(_) => StatusCode::SERVICE_UNAVAILABLE,
            Self::FailedAuthentication(err) => match err {
                AuthenticationError::FailedConnection(_) => StatusCode::SERVICE_UNAVAILABLE,
                AuthenticationError::FailedQuery(err) => query_error_status_code(err),
                _ => StatusCode::UNAUTHORIZED,
            },
            Self::JsonSerialization(err) => match err.classify() {
                Category::Io | Category::Eof => StatusCode::INTERNAL_SERVER_ERROR,
                Category::Syntax | Category::Data => StatusCode::BAD_REQUEST,
            },
            Self::FailedQuery(err) => query_error_status_code(err),
        }
    }

    fn category(&self) -> &'static str {
        match self {
            Self::AlreadyExists(_) => "Already Exists",
            Self::ContentTypeMismatch(..) => "Content Type Mismatch",
            Self::CyclicDependency(_) => "Cyclic Dependency",
            Self::DeleteDefault(_) => "Delete Default",
            Self::EmptySwf => "Empty SWF",
            Self::EmptyVideo => "Empty Video",
            Self::ExpressionFailsRegex(..) => "Expression Fails Regex",
            Self::FailedAuthentication(_) => "Failed Authentication",
            Self::FailedConnection(_) => "Failed Connection",
            Self::FailedEmailTransport(_) => "Failed Email Transport",
            Self::FailedQuery(_) => "Failed Query",
            Self::FromStr(_) => "FromStr Error",
            Self::HeaderDeserialization(_) => "Header Deserialization",
            Self::Hidden(_) => "Resource Hidden",
            Self::InsufficientPrivileges => "Insufficient Privileges",
            Self::InvalidEmailAddress(_) => "Invalid Email Address",
            Self::InvalidEmail(_) => "Invalid Email",
            Self::InvalidHeader(_) => "Invalid Header",
            Self::InvalidSort => "Invalid Sort",
            Self::InvalidTime(_) => "Invalid Time",
            Self::InvalidUserRank => "Invalid User Rank",
            Self::Image(_) => "Image Error",
            Self::JsonRejection(_) => "JSON Rejection",
            Self::JsonSerialization(_) => "JSON Serialization Error",
            Self::MissingContent(_) => "Missing Content",
            Self::MissingContentType => "Missing Content Type",
            Self::MissingFormData => "Missing Form Data",
            Self::MissingMetadata => "Missing Metadata",
            Self::MissingSmtpInfo => "Missing SMTP Info",
            Self::Multipart(_) => "Multipart/Form-Data Error",
            Self::MultipartRejection(_) => "Multipart Rejection",
            Self::NoEmail => "No Email",
            Self::NoNamesGiven(_) => "No Names Given",
            Self::NotAnInteger(_) => "Parse Int Error",
            Self::NotFound(_) => "Resource Not Found",
            Self::NotLoggedIn => "Not Logged In",
            Self::Password(_) => "Password Error",
            Self::PathRejection(_) => "Path Rejection",
            Self::QueryRejection(_) => "Query Rejection",
            Self::Request(_) => "Request Error",
            Self::ResourceModified => "Resource Modified",
            Self::SelfMerge(_) => "Self Merge",
            Self::StdIo(_) => "IO Error",
            Self::SwfDecoding(_) => "SWF Decoding Error",
            Self::UnimplementedFrameFormat(_) => "Unimplemented Frame Format",
            Self::UnauthorizedPasswordReset => "Unauthorized Password Reset",
            Self::UnsupportedExtension(_) => "Unsupported extension",
            Self::VideoDecoding(_) => "Video Decoding Error",
        }
    }

    fn response(&self) -> ErrorResponse {
        ErrorResponse {
            name: self.kind(),
            title: self.category(),
            description: self.to_string(),
        }
    }
}

impl From<LimitErrorKind> for ApiError {
    fn from(value: LimitErrorKind) -> Self {
        Self::Image(ImageError::Limits(LimitError::from(value)))
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        (self.status_code(), Json(self.response())).into_response()
    }
}

pub fn map_unique_violation<T>(result: QueryResult<T>, property: ResourceProperty) -> ApiResult<T> {
    use diesel::result::DatabaseErrorKind;
    use diesel::result::Error as DeiselError;

    match result {
        Ok(value) => Ok(value),
        Err(DeiselError::DatabaseError(DatabaseErrorKind::UniqueViolation, _)) => {
            Err(ApiError::AlreadyExists(property))
        }
        Err(err) => Err(err.into()),
    }
}

pub fn map_foreign_key_violation<T>(result: QueryResult<T>, resource: ResourceType) -> ApiResult<T> {
    use diesel::result::DatabaseErrorKind;
    use diesel::result::Error as DeiselError;

    match result {
        Ok(value) => Ok(value),
        Err(DeiselError::DatabaseError(DatabaseErrorKind::ForeignKeyViolation, _)) => Err(ApiError::NotFound(resource)),
        Err(err) => Err(err.into()),
    }
}

pub fn map_unique_or_foreign_key_violation<T>(
    result: QueryResult<T>,
    unique_property: ResourceProperty,
    foreign_resource: ResourceType,
) -> ApiResult<T> {
    use diesel::result::DatabaseErrorKind;
    use diesel::result::Error as DeiselError;

    match result {
        Ok(value) => Ok(value),
        Err(DeiselError::DatabaseError(DatabaseErrorKind::UniqueViolation, _)) => {
            Err(ApiError::AlreadyExists(unique_property))
        }
        Err(DeiselError::DatabaseError(DatabaseErrorKind::ForeignKeyViolation, _)) => {
            Err(ApiError::NotFound(foreign_resource))
        }
        Err(err) => Err(err.into()),
    }
}

/// Represents a response if an error occured.
#[derive(Serialize)]
struct ErrorResponse {
    title: &'static str,
    name: &'static str,
    description: String,
}

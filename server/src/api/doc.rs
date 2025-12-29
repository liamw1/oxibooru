use utoipa::OpenApi;

pub const COMMENT_TAG: &str = "comment";
pub const INFO_TAG: &str = "info";
pub const PASSWORD_RESET_TAG: &str = "password_reset";
pub const POOL_TAG: &str = "pool";
pub const POOL_CATEGORY_TAG: &str = "pool_category";
pub const POST_TAG: &str = "post";
pub const SNAPSHOT_TAG: &str = "snapshot";
pub const TAG_TAG: &str = "tag";
pub const TAG_CATEGORY_TAG: &str = "tag_category";
pub const UPLOAD_TAG: &str = "upload";
pub const USER_TAG: &str = "user";
pub const USER_TOKEN_TAG: &str = "user_token";

#[derive(OpenApi)]
#[openapi(
    tags(
        (name = COMMENT_TAG, description = "Comment API endpoints"),
        (name = INFO_TAG, description = "Info API endpoints"),
        (name = PASSWORD_RESET_TAG, description = "Password reset API endpoints"),
        (name = POOL_TAG, description = "Pool API endpoints"),
        (name = POOL_CATEGORY_TAG, description = "Pool category API endpoints"),
        (name = POST_TAG, description = "Post API endpoints"),
        (name = SNAPSHOT_TAG, description = "Snapshot API endpoints"),
        (name = TAG_TAG, description = "Tag API endpoints"),
        (name = TAG_CATEGORY_TAG, description = "Tag category API endpoints"),
        (name = UPLOAD_TAG, description = "Upload API endpoints"),
        (name = USER_TAG, description = "User API endpoints"),
        (name = USER_TOKEN_TAG, description = "User token API endpoints"),
        (name = "Errors", description = ERROR_DESCRIPTION),
        (name = "Field-Selection", description = FIELD_SELECTION_DESCRIPTION),
        (name = "Versioning", description = VERSIONING_DESCRIPTION)
    )
)]
pub struct ApiDoc;

const ERROR_DESCRIPTION: &str = r#"
All errors (except for unhandled fatal server errors) send relevant HTTP status
code together with JSON of following structure:

```json5
{
    "name": "Name of the error, e.g. 'PostNotFound'",
    "title": "Generic title of error message, e.g. 'Resource Not Found'",
    "description": "Detailed description of what went wrong, e.g. 'User `rr-` not found."
}
```

List of possible error names:

- `AddressInUse`
- `AddressNotAvailable`
- `AlreadyInTransaction`
- `ArgumentListTooLong`
- `BadConnection`
- `BrokenPipe`
- `BrokenTransactionManager`
- `BytesRejection`
- `CheckViolation`
- `ClosedConnection`
- `CommentNotFound`
- `ConnectionAborted`
- `ConnectionRefused`
- `ConnectionReset`
- `ContentTypeMismatch`
- `CrossesDevices`
- `CryptoError`
- `CyclicDependency`
- `Deadlock`
- `DecodeExhausted`
- `DeleteDefault`
- `DeserializationError`
- `DimensionLimitsExceeded`
- `DimensionMismatch`
- `DirectoryNotEmpty`
- `EmailAddressInvalidDomain`
- `EmailAddressInvalidInput`
- `EmailAddressInvalidUser`
- `EmailAddressMissingParts`
- `EmailAddressUnbalanced`
- `EmailCannotParseFilename`
- `EmailMissingAt`
- `EmailMissingDomain`
- `EmailMissingForm`
- `EmailMissingLocalPart`
- `EmailMissingTo`
- `EmailNonAsciiChars`
- `EmailTooManyFrom`
- `EmptySwf`
- `EmptyValue`
- `EmptyVideo`
- `EnvironmentVariableNotPresent`
- `EnvironmentVariableNotUnicode`
- `ExecutableFileBusy`
- `ExpiredToken`
- `ExpressionFailsRegex`
- `FailedAlready`
- `FailedConnection`
- `FailedDecoding`
- `FailedEmailTransport`
- `FailedEncoding`
- `FailedToDeserializeQueryString`
- `FileAlreadyExists`
- `FileNotFound`
- `FileTooLarge`
- `ForeignKeyViolation`
- `FromStrError`
- `HeaderDeserialization`
- `HostUnreachable`
- `InsufficientMemory`
- `InsufficientPrivileges`
- `Interrupted`
- `InvalidAuthType`
- `InvalidBoundary`
- `InvalidByte`
- `InvalidCharacter`
- `InvalidConnectionUrl`
- `InvalidCString`
- `InvalidData`
- `InvalidDigit`
- `InvalidEncoding`
- `InvalidExtraData`
- `InvalidFilename`
- `InvalidFormat`
- `InvalidFrameFormat`
- `InvalidHeader`
- `InvalidInput`
- `InvalidLastSymbol`
- `InvalidLength`
- `InvalidPadding`
- `InvalidPassword`
- `InvalidPhcStringField`
- `InvalidResizeParameters`
- `InvalidSort`
- `InvalidUserRank`
- `InvalidUtf8InPathParam`
- `InvalidVersion`
- `IsADirectory`
- `JsonDataError`
- `JsonInvalidData`
- `JsonInvalidSyntax`
- `JsonIoError`
- `JsonSyntaxError`
- `JsonUnexpectedEOF`
- `MalformedCredentials`
- `MalformedToken`
- `MalformedValue`
- `MissingCodecParameters`
- `MissingContent`
- `MissingContentType`
- `MissingFormData`
- `MissingJsonContentType`
- `MissingMetadata`
- `MissingPathParams`
- `MissingSmtpInfo`
- `MultipartError`
- `NegativeOverflow`
- `NetworkDown`
- `NetworkUnreachable`
- `NoEmail`
- `NoMoreData`
- `NoNamesGiven`
- `NotADirectory`
- `NotConnected`
- `NotInTransaction`
- `NotLoggedIn`
- `NotNullViolation`
- `NotSeekable`
- `OutOfMemory`
- `OutOfRange`
- `ParamNameDuplicated`
- `ParamNameInvalid`
- `ParamsMaxExceeded`
- `PathDeserializeError`
- `PathParseError`
- `PathParseErrorAtIndex`
- `PathParseErrorAtKey`
- `PermissionDenied`
- `PhcStringTrailingData`
- `PoolCategoryNameAlreadyExists`
- `PoolCategoryNotFound`
- `PoolNameAlreadyExists`
- `PoolNotFound`
- `PoolPostAlreadyExists`
- `PositiveOverflow`
- `PostAlreadyFeatured`
- `PostNotFound`
- `PostRelationAlreadyExists`
- `QueryBuilderError`
- `QuotaExceeded`
- `ReadExhausted`
- `ReadOnlyFilesystem`
- `ReadOnlyTransaction`
- `RequestError`
- `ResourceBusy`
- `ResourceHidden`
- `ResourceModified`
- `RollbackTransaction`
- `RowNotFound`
- `SelfMerge`
- `SerializationError`
- `SerializationFailure`
- `StaleNetworkFileHandle`
- `StorageFull`
- `SwfAvm1ParseError`
- `SwfInvalidData`
- `SwfIoError`
- `SwfParseError`
- `SwfUnsupported`
- `TagCategoryNameAlreadyExists`
- `TagCategoryNotFound`
- `TagNameAlreadyExists`
- `TagNotFound`
- `TimedOut`
- `TooFewArgs`
- `TooManyArgs`
- `TooManyLinks`
- `UnableToSendCommand`
- `UnauthorizedPasswordReset`
- `UnexpectedEof`
- `UnexpectedOutputSize`
- `UnimplementedFrameFormat`
- `UninitializedCodec`
- `UniqueViolation`
- `Unsupported`
- `UnsupportedAlgorithm`
- `UnsupportedCodecHardwareAccelerationDeviceType`
- `UnsupportedCodecParameterSets`
- `UnsupportedColor`
- `UnsupportedExtension`
- `UnsupportedFeature`
- `UnsupportedFormat`
- `UnsupportedImageDimensions`
- `UnsupportedPathType`
- `UserEmailAlreadyExists`
- `UserNameAlreadyExists`
- `UsernamePasswordMismatch`
- `UsernameTokenMismatch`
- `UserNotFound`
- `UserTokenNotFound`
- `Utf8ConversionError`
- `ValueTooLong`
- `ValueTooShort`
- `WouldBlock`
- `WriteRetryLimitReached`
- `WriteZero`
- `WrongNumberOfPathParameters`
- `ZeroNotAllowed`
"#;

const FIELD_SELECTION_DESCRIPTION: &str = r#"
For performance considerations, sometimes the client might want to choose the
fields the server sends to it in order to improve the query speed. This
customization is available for top-level fields of most of the
[resources](#resources). To choose the fields, the client should pass
`?fields=field1,field2,...` suffix to the query. This works regardless of the
request type (`GET`, `PUT` etc.).

For example, to list posts while getting only their IDs and tags, the client
should send a `GET` query like this:

```
GET /posts/?fields=id,tags
```
"#;

const VERSIONING_DESCRIPTION: &str = r#"
To prevent problems with concurrent resource modification, oxibooru
implements optimistic locks using resource versions. Each modifiable resource
has its `version` returned to the client with `GET` requests. At the same time,
each `PUT` and `DELETE` request sent by the client must present the same
`version` field to the server with value as it was given in `GET`.

For example, given `GET /post/1`, the server responds like this:

```
{
    ...,
    "version": 2024-09-14T19:06:56.979184564Z
}
```

This means the client must then send `{"version": 2024-09-14T19:06:56.979184564Z}`
back too. If the client fails to do so, the server will reject the request notifying
about missing parameter. If someone has edited the post in the mean time, the server
will reject the request as well, in which case the client is encouraged to notify the
user about the situation."#;

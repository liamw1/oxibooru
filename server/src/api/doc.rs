use utoipa::OpenApi;

pub const COMMENT_TAG: &str = "Comment";
pub const INFO_TAG: &str = "Info";
pub const PASSWORD_RESET_TAG: &str = "Password-Reset";
pub const POOL_TAG: &str = "Pool";
pub const POOL_CATEGORY_TAG: &str = "Pool-Category";
pub const POST_TAG: &str = "Post";
pub const SNAPSHOT_TAG: &str = "Snapshot";
pub const TAG_TAG: &str = "Tag";
pub const TAG_CATEGORY_TAG: &str = "Tag-Category";
pub const UPLOAD_TAG: &str = "Upload";
pub const USER_TAG: &str = "User";
pub const USER_TOKEN_TAG: &str = "User-Token";

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
        (name = "Authentication", description = AUTHENTICATION_DESCRIPTION),
        (name = "User-Token-Authentication", description = USER_TOKEN_AUTHENTICATION_DESCRIPTION),
        (name = "Basic-Requests", description = BASIC_REQUESTS_DESCRIPTION),
        (name = "File-Uploads", description = FILE_UPLOADS_DESCRIPTION),
        (name = "Field-Selection", description = FIELD_SELECTION_DESCRIPTION),
        (name = "Versioning", description = VERSIONING_DESCRIPTION),
        (name = "Webhooks", description = WEBHOOKS_DESCRIPTION),
        (name = "Search", description = SEARCH_DESCRIPTION),
        (name = "Errors", description = ERROR_DESCRIPTION),
    )
)]
pub struct ApiDoc;

const AUTHENTICATION_DESCRIPTION: &str = r#"
Authentication is achieved by means of [basic HTTP
auth](https://en.wikipedia.org/wiki/Basic_access_authentication) or through the
use of [user token authentication](#User-Token-Authentication). For this
reason, it is recommended to connect through HTTPS. There are no sessions, so
every privileged request must be authenticated. Available privileges depend on
the user's rank. The way how rank translates to privileges is defined in the
server's configuration.

It is recommended to add `?bump-login` GET parameter to the first request in a
client "session" (where the definition of a session is up to the client), so
that the user's last login time is kept up to date.
"#;

const USER_TOKEN_AUTHENTICATION_DESCRIPTION: &str = r#"
User token authentication works similarly to [basic HTTP
auth](https://en.wikipedia.org/wiki/Basic_access_authentication). Because it
operates similarly to ***basic HTTP auth*** it is still recommended to connect
through HTTPS. The authorization header uses the type of `Token` and the
username and token are encoded as Base64 and sent as the second parameter.

Example header for user1:token-is-more-secure
```
Authorization: Token dXNlcjE6dG9rZW4taXMtbW9yZS1zZWN1cmU=
```

The benefit of token authentication is that beyond the initial login to acquire
the first token, there is no need to transmit the user password in plaintext
via basic auth. Additionally tokens can be revoked at anytime allowing a
cleaner interface for isolating clients from user credentials.
"#;

const BASIC_REQUESTS_DESCRIPTION: &str = r#"
Every request must use `Content-Type: application/json` and `Accept:
application/json`. An exception to this rule are requests that upload files.
"#;

const FILE_UPLOADS_DESCRIPTION: &str = r#"
Requests that upload files must use `multipart/form-data` encoding. Any request
that bundles user files, must send the request data (which is JSON) as an
additional file with the special name of `metadata` with `Content-Type: application/json`
(whereas the actual files must have names specific to the API that is being used).

Alternatively, the server can download the files from the Internet on client's
behalf. In that case, the request doesn't need to be specially encoded in any
way. The files, however, should be passed as regular fields appended with a
`Url` suffix. For example, to use `http://example.com/file.jpg` in an API that
accepts a file named `content`, the client should pass
`{"contentUrl":"http://example.com/file.jpg"}` as a part of the JSON message
body. When creating or updating post content using this method, the server can
also be configured to employ [yt-dlp](https://github.com/yt-dlp/yt-dlp) to
download content from popular sites such as youtube, gfycat, etc. Access to
yt-dlp can be configured with the `'uploads:use_downloader'` permission

Finally, in some cases the user might want to reuse one file between the
requests to save the bandwidth (for example, reverse search + consecutive
upload). In this case one should use [temporary file
uploads](#Upload), and pass the tokens returned by the API as
regular fields appended with a `Token` suffix. For example, to use previously
uploaded data, which was given token `deadbeef`, in an API that accepts a file
named `content`, the client should pass `{"contentToken":"deadbeef"}` as part
of the JSON message body. If the file with the particular token doesn't exist
or it has expired, the server will show an error.
"#;

const FIELD_SELECTION_DESCRIPTION: &str = r#"
For performance considerations, sometimes the client might want to choose the
fields the server sends to it in order to improve the query speed. This
customization is available for top-level fields of most resources. To choose 
the fields, the client should pass `?fields=field1,field2,...` suffix to the 
query. This works regardless of the request type (`GET`, `PUT` etc.).

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

const WEBHOOKS_DESCRIPTION: &str = r#"
System administrators can choose to configure webhooks to track events.
Webhook URIs can be configured in `config.toml` (See `config.toml.dist` for
example). Upon any event, the API will send a `POST` request to the listed
URIs with a snapshot resource generated with anonymous user privileges as the
message body, in JSON format.
"#;

const SEARCH_DESCRIPTION: &str = r#"
Search queries are built of tokens that are separated by spaces. Each token can
be of following form:

| Syntax            | Token type        | Description                                |
| ----------------- | ----------------- | ------------------------------------------ |
| `<value>`         | anonymous tokens  | basic filters                              |
| `<key>:<value>`   | named tokens      | advanced filters                           |
| `sort:<style>`    | sort style tokens | sort the results                           |
| `special:<value>` | special tokens    | filters usually tied to the logged in user |

Anonymous and named tokens support ranged and composite values that
take following form:

| `<value>` | Description                                           |
| --------- | ----------------------------------------------------- |
| `a,b,c`   | will show things that satisfy either `a`, `b` or `c`. |
| `1..`     | will show things that are equal to or greater than 1. |
| `..4`     | will show things that are equal to at most 4.         |
| `1..4`    | will show things that are equal to 1, 2, 3 or 4.      |

Date/time values can be of following form:

- `today`
- `yesterday`
- `<year>`
- `<year>-<month>`
- `<year>-<month>-<day>`

Some fields, such as user names, can take wildcards (`*`).

All tokens can be negated by prepending them with -.

Sort style token values can be appended with ,asc or ,desc to control the sort 
direction, which can be also controlled by negating the whole token.

You can escape special characters such as `:`, `*`, and `,` by prepending them with a
backslash: `\\`.

**Example**

Searching for posts with following query:

    sea -fav-count:8.. type:png uploader:Pirate,Davy

will show png files tagged as sea, that were liked by seven people at most,
uploaded by user Pirate or Davy.

Searching for posts with `re:zero` will show an error message about unknown
named token.

Searching for posts with `re\:zero` will show posts tagged with `re:zero`.
"#;

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

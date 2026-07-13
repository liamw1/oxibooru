use crate::api::error::{ApiError, ApiResult};
use crate::app::Context;
use crate::config::Action;
use crate::content::upload::MAX_UPLOAD_SIZE;
use crate::content::upload::UploadToken;
use crate::model::enums::MimeType;
use crate::{content, filesystem};
use futures::TryStreamExt;
use mime::Mime;
use reqwest::dns::{Name, Resolve, Resolving};
use reqwest::header::{HeaderMap, HeaderValue, REFERER};
use reqwest::redirect::Policy;
use reqwest::{Client, StatusCode};
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr};
use std::str::FromStr;
use std::sync::Arc;
use std::time::Duration;
use thiserror::Error;
use tracing::warn;
use url::{Host, Url};

#[derive(Debug, Error)]
pub enum UrlValidationError {
    #[error("Address not allowed")]
    ForbiddenAddress,
    #[error("Port not allowed")]
    ForbiddenPort,
    #[error("Only https URLs are allowed")]
    ForbiddenScheme,
    #[error("URL has no host")]
    MissingHost,
}

pub fn create_client() -> reqwest::Result<Client> {
    // Some websites expect a user-agent
    const FAKE_USER_AGENT: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64; rv:135.0) Gecko/20100101 Firefox/135.0";
    const DOWNLOAD_TIMEOUT: Duration = Duration::from_mins(10);
    const CONNECTION_TIMEOUT: Duration = Duration::from_secs(10);

    Client::builder()
        .user_agent(FAKE_USER_AGENT)
        .no_proxy() // Ignore HTTP_PROXY/HTTPS_PROXY etc. A configured proxy would make connections bypass PublicOnlyResolver entirely.
        .dns_resolver(Arc::new(PublicOnlyResolver))
        .redirect(Policy::custom(|attempt| {
            if attempt.previous().len() >= MAX_REDIRECTS {
                return attempt.error("too many redirects");
            }
            // Redirect targets must also be https and not literal private IPs.
            match validate_url(attempt.url()) {
                Ok(()) => attempt.follow(),
                Err(_) => attempt.error("redirect to disallowed URL"),
            }
        }))
        .timeout(DOWNLOAD_TIMEOUT)
        .connect_timeout(CONNECTION_TIMEOUT)
        .build()
}

/// Attempts to download file at the specified `url`.
/// If successful, the file is saved in the temporary uploads directory
/// and a content token is returned.
pub async fn from_url(ctx: &Context, url: Url) -> ApiResult<UploadToken> {
    ctx.verify_privilege(Action::UploadUseDownloader)?;

    validate_url(&url)?;

    let mut response = ctx.downloader.get(url.clone()).send().await?;
    if response.status() == StatusCode::FORBIDDEN {
        let mut headers = HeaderMap::new();
        headers.insert(REFERER, HeaderValue::from_str(url.as_str())?);
        response = ctx.downloader.get(url).headers(headers).send().await?;
    }
    let response = response.error_for_status()?;

    let mime = content::parse_header(response.headers())?;
    let mime_essence = mime.as_ref().map_or("", Mime::essence_str);
    let mime_type = MimeType::from_str(mime_essence).map_err(Box::from)?;

    // Cap total bytes read; Content-Length can lie or be absent. Exceeding the cap aborts
    // the stream with an error instead of silently truncating the file.
    let mut total = 0usize;
    let limited_stream = response.bytes_stream().map_err(ApiError::from).and_then(move |chunk| {
        total += chunk.len();
        futures::future::ready(if total > MAX_UPLOAD_SIZE {
            Err(ApiError::DownloadTooLarge)
        } else {
            Ok(chunk)
        })
    });
    filesystem::save_uploaded_file(&ctx.config, limited_stream, mime_type).await
}

const MAX_REDIRECTS: usize = 5;

/// DNS resolver that filters out non-public addresses.
/// Because filtering happens *inside* resolution, redirect hops are
/// protected too, and DNS rebinding can't bypass the check.
struct PublicOnlyResolver;

impl Resolve for PublicOnlyResolver {
    fn resolve(&self, name: Name) -> Resolving {
        Box::pin(async move {
            let addrs: Vec<SocketAddr> = tokio::net::lookup_host((name.as_str(), 0))
                .await?
                .filter(|addr| ip_is_public(addr.ip()))
                .collect();
            if addrs.is_empty() {
                warn!(host = name.as_str(), "DNS resolved to no public addresses");
                Err("no public addresses resolved".into())
            } else {
                Ok(Box::new(addrs.into_iter()) as Box<dyn Iterator<Item = SocketAddr> + Send>)
            }
        })
    }
}

fn ipv4_is_public(ip: Ipv4Addr) -> bool {
    let o = ip.octets();
    !(ip.is_multicast()
        // Deprecated 6to4 relay anycast
        || matches!(ip.octets(), [192, 88, 99, _])
        // Everything past here can be replaced with `!ip.is_global()` when it stablizes (TODO)
        || ip.is_private()
        || ip.is_loopback()
        || ip.is_link_local()
        || ip.is_broadcast()
        || ip.is_documentation()
        || o[0] >= 240                                // Reserved
        || o[0] == 0                                  // On Linux 0.x.x.x can reach localhost
        || o[0] == 198 && (o[1] & 0xfe) == 18         // Benchmarking
        || o[0] == 192 && o[1] == 0 && o[2] == 0      // IETF protocol assignments
        || o[0] == 100 && (o[1] & 0b1100_0000) == 64) // CGNAT
}

fn ipv6_is_public(ip: Ipv6Addr) -> bool {
    if let Some(v4) = ip.to_ipv4() {
        return ipv4_is_public(v4);
    }
    let seg = ip.segments();
    !(ip.is_multicast()
        // Deprecated site-local
        || (seg[0] & 0xffc0) == 0xfec0
        // Everything past here can be replaced with `!ip.is_global()` when it stablizes (TODO)
        || ip.is_loopback()
        || ip.is_unspecified()
        || ip.is_unique_local()
        || ip.is_unicast_link_local()
        // IPv4-IPv6 Translat. (`64:ff9b:1::/48`)
        || matches!(ip.segments(), [0x64, 0xff9b, 1, ..])
        // Discard-Only Address Block (`100::/64`)
        || matches!(ip.segments(), [0x100, 0, 0, 0, ..])
        // IETF Protocol Assignments (`2001::/23`)
        || (matches!(ip.segments(), [0x2001, b, ..] if b < 0x200)
            && !(
                // Port Control Protocol Anycast (`2001:1::1`)
                u128::from_be_bytes(ip.octets()) == 0x2001_0001_0000_0000_0000_0000_0000_0001
                // Traversal Using Relays around NAT Anycast (`2001:1::2`)
                || u128::from_be_bytes(ip.octets()) == 0x2001_0001_0000_0000_0000_0000_0000_0002
                // AMT (`2001:3::/32`)
                || matches!(ip.segments(), [0x2001, 3, ..])
                // AS112-v6 (`2001:4:112::/48`)
                || matches!(ip.segments(), [0x2001, 4, 0x112, ..])
                // ORCHIDv2 (`2001:20::/28`)
                // Drone Remote ID Protocol Entity Tags (DETs) Prefix (`2001:30::/28`)`
                || matches!(ip.segments(), [0x2001, b, ..] if (0x20..=0x3F).contains(&b))
            ))
        // 6to4 (`2002::/16`) – it's not explicitly documented as globally reachable,
        // IANA says N/A.
        || matches!(ip.segments(), [0x2002, ..])
        // Segment Routing (SRv6) SIDs (`5f00::/16`)
        || matches!(ip.segments(), [0x5f00, ..]))
}

fn ip_is_public(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => ipv4_is_public(v4),
        IpAddr::V6(v6) => ipv6_is_public(v6),
    }
}

/// Rejects non-https schemes, disallowed ports, and literal IPs in
/// private ranges (literal IPs never hit the DNS resolver).
fn validate_url(url: &Url) -> Result<(), UrlValidationError> {
    if url.scheme() != "https" {
        return Err(UrlValidationError::ForbiddenScheme);
    }
    if !matches!(url.port(), None | Some(443)) {
        return Err(UrlValidationError::ForbiddenPort);
    }
    match url.host() {
        Some(Host::Ipv4(ip)) if !ipv4_is_public(ip) => Err(UrlValidationError::ForbiddenAddress),
        Some(Host::Ipv6(ip)) if !ipv6_is_public(ip) => Err(UrlValidationError::ForbiddenAddress),
        Some(_) => Ok(()),
        None => Err(UrlValidationError::MissingHost),
    }
}

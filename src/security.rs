use std::{
    error::Error,
    net::{IpAddr, SocketAddr},
    path::{Component, Path, PathBuf},
    time::Duration,
};

use percent_encoding::percent_decode_str;
use url::Url;

use crate::models::SourceType;

pub const MAX_URL_CHARS: usize = 8192;
const MAX_FILENAME_CHARS: usize = 200;

/// Resolver used by the HTTP client. It drops private, loopback, link-local
/// and other non-global addresses at connect time, which also closes the DNS
/// rebinding window between URL validation and the actual connection.
#[derive(Debug, Clone, Copy, Default)]
pub struct SafeDnsResolver;

impl reqwest::dns::Resolve for SafeDnsResolver {
    fn resolve(&self, name: reqwest::dns::Name) -> reqwest::dns::Resolving {
        Box::pin(async move {
            let host = name.as_str().to_owned();
            let addrs = tokio::time::timeout(
                Duration::from_secs(10),
                tokio::net::lookup_host((host.as_str(), 0)),
            )
            .await
            .map_err(|_| {
                Box::new(std::io::Error::other("DNS lookup timed out"))
                    as Box<dyn Error + Send + Sync>
            })?
            .map_err(|e| Box::new(e) as Box<dyn Error + Send + Sync>)?;
            let safe: Vec<SocketAddr> = addrs.filter(|addr| !is_forbidden_ip(addr.ip())).collect();
            if safe.is_empty() {
                return Err("host resolves only to blocked or non-public IP addresses".into());
            }
            Ok(Box::new(safe.into_iter()) as reqwest::dns::Addrs)
        })
    }
}

/// Validate an HTTP(S) URL and ensure that it points at a public destination.
/// This performs a DNS lookup for host names; the resolver enforces the same
/// policy again for every redirect and every actual connection.
pub async fn validate_public_url(raw: &str) -> Result<Url, String> {
    if raw.len() > MAX_URL_CHARS {
        return Err("URL is too long".into());
    }
    let url = Url::parse(raw.trim()).map_err(|_| "Invalid URL".to_string())?;
    if !matches!(url.scheme(), "http" | "https") {
        return Err("Only HTTP and HTTPS URLs are allowed".into());
    }
    if url.host_str().is_none() {
        return Err("URL has no host".into());
    }
    if !url.username().is_empty() || url.password().is_some() {
        return Err(
            "URLs with embedded credentials are not allowed; use Basic Authentication fields"
                .into(),
        );
    }
    validate_host(&url).await?;
    Ok(url)
}

pub async fn validate_host(url: &Url) -> Result<(), String> {
    let host = url
        .host_str()
        .ok_or_else(|| "URL has no host".to_string())?
        .to_ascii_lowercase();
    if host == "localhost"
        || host.ends_with(".localhost")
        || host == "metadata.google.internal"
        || host.ends_with(".internal")
    {
        return Err("Access to local or metadata hosts is blocked".into());
    }
    let port = url.port_or_known_default().unwrap_or(443);
    match url.host() {
        Some(url::Host::Ipv4(ip)) => {
            if is_forbidden_ip(IpAddr::V4(ip)) {
                return Err(
                    "Access to private, loopback or metadata IP addresses is blocked".into(),
                );
            }
            Ok(())
        }
        Some(url::Host::Ipv6(ip)) => {
            if is_forbidden_ip(IpAddr::V6(ip)) {
                return Err(
                    "Access to private, loopback or metadata IP addresses is blocked".into(),
                );
            }
            Ok(())
        }
        Some(url::Host::Domain(domain)) => {
            let addrs = tokio::time::timeout(
                Duration::from_secs(10),
                tokio::net::lookup_host((domain, port)),
            )
            .await
            .map_err(|_| format!("DNS lookup for {domain} timed out"))?
            .map_err(|_| format!("Could not resolve host {domain}"))?;
            let mut public = 0usize;
            for addr in addrs {
                if !is_forbidden_ip(addr.ip()) {
                    public += 1;
                }
            }
            if public == 0 {
                return Err("Host resolves only to blocked or non-public IP addresses".into());
            }
            Ok(())
        }
        None => Err("URL has no host".into()),
    }
}

pub fn is_forbidden_ip(ip: IpAddr) -> bool {
    // Explicit cloud metadata endpoints, including addresses that are
    // technically public but act as instance metadata services.
    const BLOCKED_V4: [[u8; 4]; 4] = [
        [169, 254, 169, 254], // AWS / GCP / Azure IMDS
        [100, 100, 100, 200], // Alibaba Cloud metadata
        [168, 63, 129, 16],   // Azure wire server
        [169, 254, 170, 2],   // GCP metadata alternate
    ];
    match ip {
        IpAddr::V4(v4) => {
            if BLOCKED_V4.iter().any(|candidate| candidate == &v4.octets()) {
                return true;
            }
            let [a, b, c, _d] = v4.octets();
            matches!(
                (a, b, c),
                (0, _, _)
                    | (10, _, _)
                    | (100, 64..=127, _)
                    | (127, _, _)
                    | (169, 254, _)
                    | (172, 16..=31, _)
                    | (192, 0, 0)
                    | (192, 0, 2)
                    | (192, 88, 99)
                    | (192, 168, _)
                    | (198, 18..=19, _)
                    | (198, 51, 100)
                    | (203, 0, 113)
                    | (224..=255, _, _)
            )
        }
        IpAddr::V6(v6) => {
            if v6.is_unspecified()
                || v6.is_loopback()
                || v6.is_multicast()
                || v6.is_unique_local()
                || v6.is_unicast_link_local()
            {
                return true;
            }
            if let Some(mapped) = v6.to_ipv4_mapped() {
                return is_forbidden_ip(IpAddr::V4(mapped));
            }
            let segments = v6.segments();
            // Only global unicast (2000::/3) is accepted. This rejects
            // documentation, discard, 6bone and other special-purpose blocks.
            if (segments[0] & 0xe000) != 0x2000 {
                return true;
            }
            if segments[0] == 0x2001 && segments[1] == 0x0db8 {
                return true;
            }
            if segments[0] == 0x0100 && segments[1] == 0 && segments[2] == 0 && segments[3] == 0 {
                return true;
            }
            false
        }
    }
}

pub fn detect_source(url: &Url, requested: SourceType) -> Result<SourceType, String> {
    let mega = is_mega_host(url);
    match requested {
        SourceType::Auto => Ok(if mega {
            SourceType::Mega
        } else {
            SourceType::Http
        }),
        SourceType::Mega => {
            if !mega {
                return Err("MEGA source selected, but the URL is not a MEGA share link".into());
            }
            Ok(SourceType::Mega)
        }
        SourceType::Http => {
            if mega {
                return Err("MEGA links must use the MEGA download source".into());
            }
            Ok(SourceType::Http)
        }
    }
}

pub fn is_mega_host(url: &Url) -> bool {
    let Some(host) = url.host_str() else {
        return false;
    };
    let host = host.to_ascii_lowercase();
    matches!(
        host.as_str(),
        "mega.nz" | "www.mega.nz" | "mega.co.nz" | "www.mega.co.nz" | "mega.io" | "www.mega.io"
    )
}

/// Strictly validate MEGA share URL formats before passing anything to
/// mega-get. The complete URL (including the fragment/key) is preserved when
/// invoking the process.
pub fn validate_mega_share_url(raw: &str) -> Result<Url, String> {
    if raw.len() > MAX_URL_CHARS {
        return Err("MEGA URL is too long".into());
    }
    let url = Url::parse(raw.trim()).map_err(|_| "Invalid MEGA URL".to_string())?;
    if url.scheme() != "https" {
        return Err("MEGA share links must use HTTPS".into());
    }
    if !url.username().is_empty() || url.password().is_some() {
        return Err("MEGA share links cannot contain embedded credentials".into());
    }
    if !is_mega_host(&url) {
        return Err("Not a MEGA share host".into());
    }
    let path = url.path();
    let fragment = url.fragment().unwrap_or_default();
    let modern = path.starts_with("/file/") || path.starts_with("/folder/");
    let legacy =
        fragment.starts_with('!') || fragment.starts_with("F!") || fragment.starts_with("P!");
    if !modern && !legacy {
        return Err("Unsupported MEGA share link format".into());
    }
    if modern {
        let parts: Vec<&str> = path
            .trim_start_matches('/')
            .split('/')
            .filter(|part| !part.is_empty())
            .collect();
        if parts.len() < 2 || parts[1].is_empty() {
            return Err("MEGA share link is missing its file or folder identifier".into());
        }
        // Modern links are commonly /file/<id>#<key> or /folder/<id>#<key>.
        // Some variants place the key as a third path segment. The complete
        // original URL, including the fragment, is preserved for mega-get.
        let key_in_path = parts.get(2).map(|v| !v.is_empty()).unwrap_or(false);
        if !key_in_path && fragment.is_empty() {
            return Err("MEGA share link is missing its decryption key".into());
        }
    }
    Ok(url)
}

pub fn mega_link_kind(url: &Url) -> &'static str {
    if url.path().starts_with("/folder/") || url.fragment().unwrap_or_default().starts_with("F!") {
        "folder"
    } else {
        "file"
    }
}

pub fn redact_mega_url(_raw: &str) -> String {
    "https://mega.nz/<redacted-share-link>".to_string()
}

pub fn redact_url(raw: &str) -> String {
    match Url::parse(raw) {
        Ok(mut url) => {
            let _ = url.set_username("");
            let _ = url.set_password(None);
            url.set_query(None);
            url.set_fragment(None);
            url.to_string()
        }
        Err(_) => "<invalid-url>".into(),
    }
}

/// Return a safe file name that contains no path separators or control chars.
pub fn sanitize_filename(input: &str) -> String {
    let name = input
        .rsplit(['/', '\\'])
        .next()
        .unwrap_or(input)
        .trim()
        .trim_matches('.')
        .chars()
        .map(|c| {
            if c.is_control()
                || matches!(
                    c,
                    '\0' | '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|'
                )
            {
                '_'
            } else {
                c
            }
        })
        .collect::<String>();
    let name = name.trim();
    let name = if name.is_empty() || name == "." || name == ".." {
        "download".to_string()
    } else {
        name.to_string()
    };
    truncate_chars(&name, MAX_FILENAME_CHARS)
}

fn truncate_chars(value: &str, max: usize) -> String {
    if value.chars().count() <= max {
        return value.to_string();
    }
    let mut out = value.chars().take(max).collect::<String>();
    out = out.trim_end_matches([' ', '.']).to_string();
    if out.is_empty() {
        "download".into()
    } else {
        out
    }
}

/// Parse a Content-Disposition filename, supporting RFC 5987 `filename*`.
pub fn filename_from_content_disposition(value: &str) -> Option<String> {
    let value = value.trim();
    for part in split_header_params(value) {
        let lower = part.to_ascii_lowercase();
        if let Some(rest) = lower.strip_prefix("filename*=") {
            let original = &part[part.len() - rest.len()..];
            let decoded = decode_rfc5987(original);
            if let Some(name) = decoded.filter(|v| !v.trim().is_empty()) {
                return Some(sanitize_filename(&name));
            }
        }
    }
    for part in split_header_params(value) {
        let lower = part.to_ascii_lowercase();
        if let Some(rest) = lower.strip_prefix("filename=") {
            let original = &part[part.len() - rest.len()..];
            let unquoted = original
                .trim()
                .trim_matches('"')
                .replace("\\\"", "\"")
                .replace("\\\\", "\\");
            if !unquoted.trim().is_empty() {
                return Some(sanitize_filename(&unquoted));
            }
        }
    }
    None
}

fn split_header_params(value: &str) -> Vec<String> {
    let mut parts = Vec::new();
    let mut current = String::new();
    let mut quoted = false;
    let mut escaped = false;
    for c in value.chars() {
        if escaped {
            current.push(c);
            escaped = false;
        } else if c == '\\' && quoted {
            current.push(c);
            escaped = true;
        } else if c == '"' {
            quoted = !quoted;
            current.push(c);
        } else if c == ';' && !quoted {
            parts.push(current.trim().to_string());
            current.clear();
        } else {
            current.push(c);
        }
    }
    if !current.trim().is_empty() {
        parts.push(current.trim().to_string());
    }
    parts
}

fn decode_rfc5987(value: &str) -> Option<String> {
    let value = value.trim().trim_matches('"');
    let mut chunks = value.splitn(3, '\'');
    let charset = chunks.next()?;
    let _language = chunks.next()?;
    let encoded = chunks.next()?;
    if !charset.eq_ignore_ascii_case("utf-8") {
        return None;
    }
    percent_decode_str(encoded)
        .decode_utf8()
        .ok()
        .map(|v| v.into_owned())
}

pub fn filename_from_url(url: &Url) -> Option<String> {
    let last = url.path_segments()?.rfind(|s| !s.is_empty())?;
    let decoded = percent_decode_str(last).decode_utf8().ok()?;
    let name = sanitize_filename(&decoded);
    if name == "download" {
        None
    } else {
        Some(name)
    }
}

/// Normalize an S3 object key. Rejects traversal and absolute paths. The
/// returned key never starts with a slash.
pub fn normalize_object_key(raw: &str) -> Result<String, String> {
    let raw = raw.trim().replace('\\', "/");
    if raw.chars().count() > 1024 {
        return Err("Object path is too long".into());
    }
    let mut parts = Vec::new();
    for segment in raw.split('/') {
        let segment = segment.trim();
        if segment.is_empty() || segment == "." {
            continue;
        }
        if segment == ".." {
            return Err("Object path cannot contain '..'".into());
        }
        if segment.contains('\0') || segment.chars().any(|c| c.is_control()) {
            return Err("Object path contains control characters".into());
        }
        parts.push(segment.to_string());
    }
    if parts.is_empty() {
        return Err("Object path is empty".into());
    }
    Ok(parts.join("/"))
}

/// Append a relative folder path to an object prefix while keeping it safe.
pub fn join_object_key(prefix: &str, relative: &Path) -> Result<String, String> {
    let base = normalize_object_key(prefix)?;
    let mut out = base;
    for component in relative.components() {
        match component {
            Component::Normal(part) => {
                let part = part.to_string_lossy();
                let part = part.trim();
                if part.is_empty() || part == "." || part == ".." {
                    return Err("Unsafe directory entry in download".into());
                }
                out.push('/');
                out.push_str(part);
            }
            _ => return Err("Unsafe directory entry in download".into()),
        }
    }
    if out.len() > 1024 {
        return Err("Generated object path is too long".into());
    }
    Ok(out)
}

/// Join a relative path under a root without allowing escape via `..`.
pub fn safe_join(root: &Path, relative: &Path) -> Result<PathBuf, String> {
    let mut out = PathBuf::from(root);
    for component in relative.components() {
        match component {
            Component::Normal(part) => out.push(part),
            Component::CurDir => {}
            _ => return Err("Path escapes the task directory".into()),
        }
    }
    Ok(out)
}

pub fn display_bytes(value: i64) -> String {
    const UNITS: [&str; 5] = ["B", "KiB", "MiB", "GiB", "TiB"];
    let mut size = value.max(0) as f64;
    let mut unit = 0;
    while size >= 1024.0 && unit < UNITS.len() - 1 {
        size /= 1024.0;
        unit += 1;
    }
    if unit == 0 {
        format!("{} {}", size as i64, UNITS[unit])
    } else {
        format!("{size:.1} {}", UNITS[unit])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn blocks_private_and_metadata_ips() {
        for ip in [
            "127.0.0.1",
            "10.0.0.1",
            "172.16.0.1",
            "192.168.1.1",
            "169.254.169.254",
            "100.100.100.200",
            "168.63.129.16",
            "::1",
            "fd00::1",
            "fe80::1",
        ] {
            assert!(is_forbidden_ip(ip.parse().unwrap()), "{ip}");
        }
        assert!(!is_forbidden_ip("1.1.1.1".parse().unwrap()));
        assert!(!is_forbidden_ip("2606:4700:4700::1111".parse().unwrap()));
    }

    #[test]
    fn sanitizes_paths_and_names() {
        assert_eq!(sanitize_filename("../../etc/passwd"), "passwd");
        assert_eq!(sanitize_filename(".."), "download");
        assert_eq!(sanitize_filename("a\0b"), "a_b");
        assert!(normalize_object_key("../../etc/passwd").is_err());
        assert_eq!(
            normalize_object_key("/archive//file.zip").unwrap(),
            "archive/file.zip"
        );
    }

    #[test]
    fn parses_content_disposition() {
        assert_eq!(
            filename_from_content_disposition("attachment; filename=\"hello.zip\""),
            Some("hello.zip".into())
        );
        assert_eq!(
            filename_from_content_disposition("attachment; filename*=UTF-8''na%C3%AFve%20file.zip"),
            Some("naïve file.zip".into())
        );
    }

    #[test]
    fn validates_mega_links() {
        assert!(validate_mega_share_url("https://mega.nz/file/abc123#key123").is_ok());
        assert!(validate_mega_share_url("https://mega.nz/folder/abc123#key123").is_ok());
        assert!(validate_mega_share_url("https://mega.nz/#!abc!key").is_ok());
        assert!(validate_mega_share_url("https://mega.nz/file/abc123#P!encrypted").is_ok());
        assert!(validate_mega_share_url("https://mega.nz/#P!encrypted").is_ok());
        assert!(validate_mega_share_url("https://example.com/file/a#b").is_err());
        assert!(validate_mega_share_url("https://mega.nz/file/abc").is_err());
    }
}

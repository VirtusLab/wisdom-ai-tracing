//! Validation for user-supplied credential `base_url`s. The server makes
//! outbound requests to this URL, so it is an SSRF surface: reject anything
//! that isn't a plain https URL to a public host.

use std::net::IpAddr;

use crate::error::AppError;

/// Validate a user-supplied base URL. Requires `https`, a host, and rejects
/// hosts that are IP literals in private/loopback/link-local ranges. Hostnames
/// are allowed (DNS rebinding is out of scope for step 1; the blast radius is
/// the user's own key). Returns the normalized URL (trailing slash trimmed).
pub fn validate_base_url(raw: &str) -> Result<String, AppError> {
    let url = url::Url::parse(raw)
        .map_err(|_| AppError::BadRequest(format!("base_url is not a valid URL: {raw}")))?;

    if url.scheme() != "https" {
        return Err(AppError::BadRequest("base_url must use https".into()));
    }

    let host = url
        .host_str()
        .ok_or_else(|| AppError::BadRequest("base_url must include a host".into()))?;

    if let Ok(ip) = host.parse::<IpAddr>() {
        if is_blocked_ip(ip) {
            return Err(AppError::BadRequest(
                "base_url must not point at a private or loopback address".into(),
            ));
        }
    }

    Ok(raw.trim_end_matches('/').to_string())
}

fn is_blocked_ip(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => {
            v4.is_private()
                || v4.is_loopback()
                || v4.is_link_local()
                || v4.is_unspecified()
                || v4.octets()[0] == 0
        }
        IpAddr::V6(v6) => {
            v6.is_loopback()
                || v6.is_unspecified()
                || v6.is_unique_local()
                || v6.is_unicast_link_local()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::validate_base_url;

    #[test]
    fn accepts_public_https() {
        assert_eq!(
            validate_base_url("https://api.anthropic.com/").unwrap(),
            "https://api.anthropic.com"
        );
    }

    #[test]
    fn rejects_http() {
        assert!(validate_base_url("http://api.anthropic.com").is_err());
    }

    #[test]
    fn rejects_loopback_and_private_and_metadata() {
        assert!(validate_base_url("https://127.0.0.1").is_err());
        assert!(validate_base_url("https://10.0.0.5").is_err());
        assert!(validate_base_url("https://192.168.1.1").is_err());
        assert!(validate_base_url("https://169.254.169.254").is_err());
    }

    #[test]
    fn rejects_garbage_and_missing_host() {
        assert!(validate_base_url("not a url").is_err());
        assert!(validate_base_url("https://").is_err());
    }
}

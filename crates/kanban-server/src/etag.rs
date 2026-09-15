use crate::error::AppError;
use axum::http::{header, HeaderMap, HeaderValue, StatusCode};
use axum::response::{IntoResponse, Response};
use kanban_service::api::{ApiError, ErrorCode};
use serde::Serialize;
use sha2::{Digest, Sha256};

const HEX: [u8; 16] = *b"0123456789abcdef";

pub fn etag_for(body: &[u8]) -> String {
    let digest = Sha256::digest(body);
    let mut tag = String::with_capacity(34);
    tag.push('"');
    for byte in &digest[..16] {
        tag.push(HEX[(byte >> 4) as usize] as char);
        tag.push(HEX[(byte & 0x0f) as usize] as char);
    }
    tag.push('"');
    tag
}

fn strong(tag: &str) -> &str {
    tag.strip_prefix("W/").unwrap_or(tag)
}

pub fn if_none_match_matches(headers: &HeaderMap, etag: &str) -> bool {
    let ours = strong(etag);
    headers
        .get_all(header::IF_NONE_MATCH)
        .iter()
        .filter_map(|value| value.to_str().ok())
        .flat_map(|value| value.split(','))
        .map(str::trim)
        .any(|candidate| candidate == "*" || strong(candidate) == ours)
}

fn serialization_failed() -> AppError {
    AppError(ApiError::new(
        ErrorCode::SerializationError,
        "failed to serialize response",
    ))
}

fn precondition_failed() -> AppError {
    AppError(ApiError::new(
        ErrorCode::PreconditionFailed,
        "the representation this request was conditioned on is no longer current",
    ))
}

fn if_match_matches(headers: &HeaderMap, current: Option<&str>) -> bool {
    let Some(current) = current else {
        return false;
    };
    headers
        .get_all(header::IF_MATCH)
        .iter()
        .filter_map(|value| value.to_str().ok())
        .flat_map(|value| value.split(','))
        .map(str::trim)
        .any(|candidate| candidate == "*" || candidate == current)
}

/// RFC 9110 13.1.1 `If-Match`: strong comparison, so a `W/`-prefixed tag never
/// matches; `*` matches any current representation. An absent header proceeds
/// without calling `current`. `None` from `current` means the target has no
/// representation yet, which fails every `If-Match`, including `*`.
pub fn check_if_match<T: Serialize>(
    headers: &HeaderMap,
    current: impl FnOnce() -> Result<Option<T>, AppError>,
) -> Result<(), AppError> {
    if !headers.contains_key(header::IF_MATCH) {
        return Ok(());
    }
    let tag = match current()? {
        Some(value) => Some(etag_for(
            &serde_json::to_vec(&value).map_err(|_| serialization_failed())?,
        )),
        None => None,
    };
    if if_match_matches(headers, tag.as_deref()) {
        Ok(())
    } else {
        Err(precondition_failed())
    }
}

pub fn json_with_etag<T: Serialize>(headers: &HeaderMap, value: &T) -> Result<Response, AppError> {
    let body = serde_json::to_vec(value).map_err(|_| serialization_failed())?;
    let tag = etag_for(&body);
    let tag_value = HeaderValue::try_from(tag).map_err(|_| serialization_failed())?;

    let mut response = if if_none_match_matches(headers, tag_value.to_str().unwrap_or_default()) {
        StatusCode::NOT_MODIFIED.into_response()
    } else {
        let mut response = body.into_response();
        response.headers_mut().insert(
            header::CONTENT_TYPE,
            HeaderValue::from_static("application/json"),
        );
        response
    };
    response.headers_mut().insert(header::ETAG, tag_value);
    Ok(response)
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::HeaderMap;

    #[test]
    fn test_etag_is_quoted_thirty_two_hex_chars() {
        let tag = etag_for(b"{}");
        assert_eq!(tag.len(), 34);
        assert!(tag.starts_with('"'));
        assert!(tag.ends_with('"'));
        assert!(tag[1..33]
            .chars()
            .all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase()));
    }

    #[test]
    fn test_same_body_same_etag_different_body_different_etag() {
        assert_eq!(etag_for(b"{\"a\":1}"), etag_for(b"{\"a\":1}"));
        assert_ne!(etag_for(b"{\"a\":1}"), etag_for(b"{\"a\":2}"));
    }

    #[test]
    fn test_if_none_match_star_matches_any_etag() {
        let mut headers = HeaderMap::new();
        headers.insert("if-none-match", "*".parse().unwrap());
        assert!(if_none_match_matches(&headers, "\"abc\""));
    }

    #[test]
    fn test_if_none_match_ignores_weak_prefix() {
        let mut headers = HeaderMap::new();
        headers.insert("if-none-match", "W/\"abc\"".parse().unwrap());
        assert!(if_none_match_matches(&headers, "\"abc\""));

        let mut headers = HeaderMap::new();
        headers.insert("if-none-match", "\"abc\"".parse().unwrap());
        assert!(if_none_match_matches(&headers, "\"abc\""));
    }

    #[test]
    fn test_if_none_match_matches_any_tag_in_comma_list() {
        let mut headers = HeaderMap::new();
        headers.insert(
            "if-none-match",
            "\"zzz\", W/\"abc\" , \"yyy\"".parse().unwrap(),
        );
        assert!(if_none_match_matches(&headers, "\"abc\""));
    }

    #[test]
    fn test_if_none_match_with_only_non_matching_tags_does_not_match() {
        let mut headers = HeaderMap::new();
        headers.insert("if-none-match", "\"zzz\", \"yyy\"".parse().unwrap());
        assert!(!if_none_match_matches(&headers, "\"abc\""));
    }

    #[test]
    fn test_absent_if_none_match_never_matches() {
        let headers = HeaderMap::new();
        assert!(!if_none_match_matches(&headers, "\"\""));
        assert!(!if_none_match_matches(&headers, "\"abc\""));
    }

    #[derive(Serialize)]
    struct Dto {
        a: u32,
    }

    #[test]
    fn test_absent_if_match_passes_without_building_the_representation() {
        let headers = HeaderMap::new();
        let result = check_if_match(&headers, || -> Result<Option<Dto>, AppError> {
            panic!("representation must not be built")
        });
        assert!(result.is_ok());
    }

    #[test]
    fn test_matching_if_match_passes() {
        let mut headers = HeaderMap::new();
        let tag = etag_for(b"{\"a\":1}");
        headers.insert("if-match", tag.parse().unwrap());
        let result = check_if_match(&headers, || Ok(Some(Dto { a: 1 })));
        assert!(result.is_ok());
    }

    #[test]
    fn test_stale_if_match_fails_with_precondition_failed() {
        let mut headers = HeaderMap::new();
        headers.insert("if-match", "\"not-the-current-tag\"".parse().unwrap());
        let result = check_if_match(&headers, || Ok(Some(Dto { a: 1 })));
        let err = result.expect_err("stale tag must fail");
        assert_eq!(err.0.code, ErrorCode::PreconditionFailed);
        assert_eq!(err.0.code.http_status(), 412);
    }

    #[test]
    fn test_star_if_match_passes_when_a_representation_exists() {
        let mut headers = HeaderMap::new();
        headers.insert("if-match", "*".parse().unwrap());
        let result = check_if_match(&headers, || Ok(Some(Dto { a: 1 })));
        assert!(result.is_ok());
    }

    #[test]
    fn test_star_if_match_fails_when_no_representation_exists() {
        let mut headers = HeaderMap::new();
        headers.insert("if-match", "*".parse().unwrap());
        let result = check_if_match(&headers, || Ok(None::<Dto>));
        let err = result.expect_err("star with no representation must fail");
        assert_eq!(err.0.code, ErrorCode::PreconditionFailed);
    }

    #[test]
    fn test_weak_if_match_tag_never_matches() {
        let mut headers = HeaderMap::new();
        let tag = etag_for(b"{\"a\":1}");
        headers.insert("if-match", format!("W/{tag}").parse().unwrap());
        let result = check_if_match(&headers, || Ok(Some(Dto { a: 1 })));
        let err = result.expect_err("weak tag must not satisfy If-Match");
        assert_eq!(err.0.code, ErrorCode::PreconditionFailed);
    }

    #[test]
    fn test_if_match_matches_any_strong_tag_in_a_comma_list() {
        let tag = etag_for(b"{\"a\":1}");

        let mut headers = HeaderMap::new();
        headers.insert(
            "if-match",
            format!("\"zzz\", {tag}, \"yyy\"").parse().unwrap(),
        );
        let result = check_if_match(&headers, || Ok(Some(Dto { a: 1 })));
        assert!(result.is_ok());

        let mut headers = HeaderMap::new();
        headers.insert("if-match", format!("W/{tag}, \"yyy\"").parse().unwrap());
        let result = check_if_match(&headers, || Ok(Some(Dto { a: 1 })));
        assert!(result.is_err());
    }
}

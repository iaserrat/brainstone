use reqwest::StatusCode;

use crate::failure::{
    CODE_AUTHENTICATION, CODE_CONFLICT, CODE_HTTP, CODE_INVALID_REQUEST, CODE_NOT_FOUND,
    CODE_PERMISSION, CODE_PROVIDER_UNAVAILABLE, CODE_RATE_LIMITED,
};

pub(crate) fn retryable(status: StatusCode) -> bool {
    matches!(status.as_u16(), 408 | 409 | 429 | 500 | 502 | 503 | 504)
}

pub(crate) fn code(status: StatusCode) -> &'static str {
    match status.as_u16() {
        400 | 413 | 422 => CODE_INVALID_REQUEST,
        401 => CODE_AUTHENTICATION,
        403 => CODE_PERMISSION,
        404 => CODE_NOT_FOUND,
        408 => crate::CODE_TIMEOUT,
        409 => CODE_CONFLICT,
        429 => CODE_RATE_LIMITED,
        500..=599 => CODE_PROVIDER_UNAVAILABLE,
        _ => CODE_HTTP,
    }
}

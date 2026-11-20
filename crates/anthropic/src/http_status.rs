use reqwest::StatusCode;

use crate::failure::{
    AUTHENTICATION_FAILED, BILLING_ERROR, CONFLICT, HTTP_ERROR, INVALID_REQUEST, NOT_FOUND,
    PERMISSION_DENIED, PROVIDER_UNAVAILABLE, RATE_LIMITED, REQUEST_TIMEOUT,
};

pub(crate) fn retryable(status: StatusCode) -> bool {
    matches!(status.as_u16(), 408 | 429 | 500 | 502 | 503 | 504 | 529)
}

pub(crate) fn code(status: StatusCode) -> &'static str {
    match status.as_u16() {
        400 | 413 => INVALID_REQUEST,
        401 => AUTHENTICATION_FAILED,
        402 => BILLING_ERROR,
        403 => PERMISSION_DENIED,
        404 => NOT_FOUND,
        408 | 504 => REQUEST_TIMEOUT,
        409 => CONFLICT,
        429 => RATE_LIMITED,
        500..=599 => PROVIDER_UNAVAILABLE,
        _ => HTTP_ERROR,
    }
}

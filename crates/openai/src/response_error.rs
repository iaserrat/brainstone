//! What a `failed` response reports, in the codes consumers branch on.

use serde::Deserialize;

use crate::failure::{CODE_PROVIDER_FAILED, Failure};

#[derive(Deserialize)]
pub(crate) struct ApiError {
    code: Option<String>,
    message: String,
}

pub(crate) fn failure(error: Option<&ApiError>) -> Failure {
    let Some(error) = error else {
        return Failure::new(CODE_PROVIDER_FAILED, "the provider failed");
    };
    let code = crate::failure::classified(error.code.as_deref(), CODE_PROVIDER_FAILED);
    Failure::new(code, error.message.clone())
}

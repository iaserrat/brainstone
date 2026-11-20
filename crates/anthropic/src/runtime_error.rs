//! Bedrock runtime exceptions mapped onto this crate's stable codes.

use aws_sdk_bedrockruntime::error::{DisplayErrorContext, ProvideErrorMetadata, SdkError};
use aws_sdk_bedrockruntime::operation::invoke_model_with_response_stream::InvokeModelWithResponseStreamError as Invoke;
use aws_sdk_bedrockruntime::types::error::ResponseStreamError as Stream;

use crate::failure::{
    Failure, INVALID_REQUEST, NOT_FOUND, PERMISSION_DENIED, PROVIDER_FAILED, PROVIDER_UNAVAILABLE,
    RATE_LIMITED, REQUEST_TIMEOUT, TRANSPORT_ERROR,
};

pub(crate) fn invoke_failure<R: std::fmt::Debug>(error: &SdkError<Invoke, R>) -> Failure {
    failure(error, invoke_code)
}

pub(crate) fn stream_failure<R: std::fmt::Debug>(error: &SdkError<Stream, R>) -> Failure {
    failure(error, stream_code)
}

fn failure<E, R>(error: &SdkError<E, R>, service_code: fn(&E) -> &'static str) -> Failure
where
    E: ProvideErrorMetadata + std::error::Error + 'static,
    R: std::fmt::Debug,
{
    let code = match error {
        SdkError::TimeoutError(_) => REQUEST_TIMEOUT,
        SdkError::DispatchFailure(_) => TRANSPORT_ERROR,
        SdkError::ServiceError(service) => service_code(service.err()),
        _ => PROVIDER_FAILED,
    };
    let message = error
        .message()
        .map_or_else(|| DisplayErrorContext(error).to_string(), str::to_owned);
    Failure::new(code, message)
}

fn invoke_code(error: &Invoke) -> &'static str {
    match error {
        Invoke::ThrottlingException(_)
        | Invoke::ModelNotReadyException(_)
        | Invoke::ServiceQuotaExceededException(_) => RATE_LIMITED,
        Invoke::AccessDeniedException(_) => PERMISSION_DENIED,
        Invoke::ValidationException(_) => INVALID_REQUEST,
        Invoke::ResourceNotFoundException(_) => NOT_FOUND,
        Invoke::ModelTimeoutException(_) => REQUEST_TIMEOUT,
        Invoke::InternalServerException(_) | Invoke::ServiceUnavailableException(_) => {
            PROVIDER_UNAVAILABLE
        }
        _ => PROVIDER_FAILED,
    }
}

fn stream_code(error: &Stream) -> &'static str {
    match error {
        Stream::ThrottlingException(_) => RATE_LIMITED,
        Stream::ValidationException(_) => INVALID_REQUEST,
        Stream::ModelTimeoutException(_) => REQUEST_TIMEOUT,
        Stream::InternalServerException(_) | Stream::ServiceUnavailableException(_) => {
            PROVIDER_UNAVAILABLE
        }
        _ => PROVIDER_FAILED,
    }
}

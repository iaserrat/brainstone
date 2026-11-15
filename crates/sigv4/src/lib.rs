//! AWS `SigV4` identity and request signing, shared by every adapter that
//! reaches Amazon Bedrock over plain HTTP.

#![deny(missing_docs)]
mod error;
mod identity;
mod sign;
#[cfg(test)]
mod sign_tests;

pub use error::Error;
pub use identity::SigV4;

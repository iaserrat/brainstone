//! Images a tool or surface hands the model: untrusted bytes validated and
//! fitted to the model's limits, the stable reference a reply uses to name one
//! so a surface can attach it, and the tool that lets a model write a file.

#![deny(missing_docs)]
mod create_file;
mod decode;
mod encode;
mod error;
mod file;
mod fit;
mod reference;

pub use create_file::CreateFile;
pub use decode::{MAX_IMAGE_BYTES, decode, media_type};
pub use encode::png;
pub use error::MediaError;
pub use file::{FILE_REFERENCE_PREFIX, MAX_FILE_BYTES, file_attachment, file_reference};
pub use reference::{REFERENCE_PREFIX, attachment, reference};

//! Surface-neutral thread input: the files a conversation carries, the caps
//! and validation that turn them into model input, and how they render.
//! A surface implements [`ThreadSource`]; everything else is shared. A surface
//! implements [`Destinations`] to let a schedule post in its conversations.

#![deny(missing_docs)]
mod attachment;
mod attachment_collect;
mod attachment_error;
mod attachment_fetch;
mod attachment_plan;
mod attachment_render;
mod destination;
mod file;
mod image_validate;
mod read_file_tool;
mod read_thread_tool;
mod source;
mod thread;
mod thread_files;
mod tool_render;
mod tool_thread;
mod window;
mod window_render;

#[cfg(test)]
mod attachment_tests;
#[cfg(test)]
mod fake_source;
#[cfg(test)]
mod image_tests;
#[cfg(test)]
mod thread_files_tests;
#[cfg(test)]
mod tool_tests;
#[cfg(test)]
mod window_tests;

pub use attachment::{FileStatus, Materialized, SkippedAttachment, TextAttachment};
pub use attachment_error::AttachmentError;
pub use attachment_fetch::materialize;
pub use destination::{Destinations, PostRequest};
pub use file::FileRef;
pub use read_file_tool::ReadThreadFile;
pub use read_thread_tool::ReadThread;
pub use source::{Fetched, ThreadSource};
pub use thread::{MAX_THREAD_MESSAGES, Reaction, ThreadHistory, ThreadMessage};
pub use thread_files::materialize_thread;
pub use tool_thread::thread_target;
pub use window::{Window, unseen};
pub use window_render::{entries, sanitize};

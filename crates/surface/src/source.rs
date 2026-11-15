use runtime::DeliveryTarget;

use crate::{AttachmentError, FileRef, ThreadHistory};

/// One file's fetch result. A per-file failure is data the model is told
/// about; only a systemic failure is the outer `Err`.
pub type Fetched<T> = Result<T, AttachmentError>;

/// What a surface implements so the agent receives its thread's files. The
/// caps, validation, and rendering around it are shared by every surface.
pub trait ThreadSource: Send + Sync {
    /// A systemic failure (transport, authentication, rate limit). It aborts
    /// materialization so the surface can retry the whole prompt.
    type Error: Send;

    /// Every message of the thread `target` routes to, oldest first, read up
    /// to [`crate::MAX_THREAD_MESSAGES`].
    fn messages(
        &self,
        target: &DeliveryTarget,
    ) -> impl Future<Output = Result<ThreadHistory, Self::Error>> + Send;

    /// The complete metadata of a file whose copy was partial.
    fn describe(
        &self,
        file: &FileRef,
    ) -> impl Future<Output = Result<Fetched<FileRef>, Self::Error>> + Send;

    /// The file's bytes; a download past `cap` fails as `TooLarge`.
    fn bytes(
        &self,
        file: &FileRef,
        cap: usize,
    ) -> impl Future<Output = Result<Fetched<Vec<u8>>, Self::Error>> + Send;
}

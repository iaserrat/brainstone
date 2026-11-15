//! Eager thread files share the prompt's caps; table-driven.
#![allow(clippy::too_many_lines)]

use std::io::Cursor;

use image::{DynamicImage, ImageFormat};

use crate::attachment_plan::MAX_IMAGES;
use crate::fake_source::{Fake, file};
use crate::{FileRef, FileStatus, ThreadMessage, Window, materialize_thread};

fn png() -> Vec<u8> {
    let mut bytes = Cursor::new(Vec::new());
    DynamicImage::new_luma8(2, 3)
        .write_to(&mut bytes, ImageFormat::Png)
        .unwrap();
    bytes.into_inner()
}

fn upload(id: &str, files: Vec<FileRef>) -> ThreadMessage {
    ThreadMessage {
        id: id.to_owned(),
        author: "U2".to_owned(),
        own: false,
        text: String::new(),
        files,
        reactions: Vec::new(),
    }
}

fn window(messages: Vec<ThreadMessage>) -> Window {
    Window {
        messages,
        omitted: 0,
    }
}

fn image(id: &str) -> FileRef {
    file(id, &format!("{id}.png"), "image/png", png().len())
}

// The field case: a screenshot posted just before the mention reaches the
// model as an image input, not as a caption.
#[tokio::test]
async fn a_screenshot_in_an_unseen_message_is_attached() {
    let shot = image("F1");
    let source = Fake::serving(&[(&shot, &png())]);
    let window = window(vec![upload("172.004", vec![shot])]);
    let input = materialize_thread(&[], &window, &source).await.unwrap();
    assert_eq!(input.images.len(), 1);
    assert_eq!(input.statuses["F1"], FileStatus::Attached);
    assert!(input.skipped.is_empty());
}

// The prompt's own files take the slots first, then the newest thread files;
// what does not fit stays fetchable instead of being called skipped.
#[tokio::test]
async fn the_prompt_goes_first_and_the_newest_thread_files_fill_the_rest() {
    let prompt_files: Vec<FileRef> = (0..MAX_IMAGES - 1)
        .map(|index| image(&format!("P{index}")))
        .collect();
    let (old, new) = (image("F1"), image("F2"));
    let png = png();
    let mut served: Vec<(&FileRef, &[u8])> = prompt_files.iter().map(|f| (f, &png[..])).collect();
    served.extend([(&old, &png[..]), (&new, &png[..])]);
    let source = Fake::serving(&served);
    let window = window(vec![
        upload("172.003", vec![old]),
        upload("172.004", vec![new]),
    ]);
    let input = materialize_thread(&prompt_files, &window, &source)
        .await
        .unwrap();
    assert_eq!(input.images.len(), MAX_IMAGES);
    assert_eq!(input.statuses["F2"], FileStatus::Attached);
    assert_eq!(input.statuses["F1"], FileStatus::Available);
    assert!(input.skipped.is_empty(), "{:?}", input.skipped);
}

// A thread file's failure is reported on its own line, never as a skip note
// about the prompt's attachments.
#[tokio::test]
async fn a_thread_file_failure_is_its_status_not_a_prompt_skip_note() {
    let archive = file("F3", "data.zip", "application/zip", 9);
    let window = window(vec![upload("172.004", vec![archive])]);
    let input = materialize_thread(&[], &window, &Fake::default())
        .await
        .unwrap();
    assert_eq!(
        input.statuses["F3"],
        FileStatus::Skipped(crate::AttachmentError::Unsupported)
    );
    assert!(input.skipped.is_empty());
    assert!(input.block().is_none());
}

// A file shared twice in the thread, or also attached to the prompt, is
// downloaded and attached once.
#[tokio::test]
async fn a_file_shared_twice_is_materialized_once() {
    let shot = image("F1");
    let source = Fake::serving(&[(&shot, &png())]);
    let window = window(vec![
        upload("172.003", vec![shot.clone()]),
        upload("172.004", vec![shot.clone()]),
    ]);
    let input = materialize_thread(&[shot], &window, &source).await.unwrap();
    assert_eq!(input.images.len(), 1);
    assert_eq!(source.downloads.lock().unwrap().len(), 1);
    assert_eq!(input.statuses["F1"], FileStatus::Attached);
}

// Overflow from the prompt's own files still becomes skip notes, exactly as
// before thread files existed.
#[tokio::test]
async fn the_prompts_own_overflow_keeps_its_skip_notes() {
    let prompt_files: Vec<FileRef> = (0..=MAX_IMAGES)
        .map(|index| image(&format!("P{index}")))
        .collect();
    let png = png();
    let served: Vec<(&FileRef, &[u8])> = prompt_files.iter().map(|f| (f, &png[..])).collect();
    let source = Fake::serving(&served);
    let input = materialize_thread(&prompt_files, &window(Vec::new()), &source)
        .await
        .unwrap();
    assert_eq!(input.images.len(), MAX_IMAGES);
    assert_eq!(input.skipped[0].error.code(), "too_many_attachments");
}

// A file whose copy was partial is described once; its line then shows what
// the surface resolved, not the blanks it started with.
#[tokio::test]
async fn a_described_file_renders_its_resolved_type_and_size() {
    let complete = image("F1");
    let partial = FileRef {
        media_type: None,
        size: None,
        url: None,
        ..complete.clone()
    };
    let mut source = Fake::serving(&[(&complete, &png())]);
    source.described.insert("F1".to_owned(), complete);
    let window = window(vec![upload("172.004", vec![partial])]);
    let input = materialize_thread(&[], &window, &source).await.unwrap();
    let rendered = crate::entries(&window, &input, str::to_owned);
    let expected = format!(
        "type=\"image/png\" size=\"{}\" status=\"attached\"",
        png().len()
    );
    assert!(rendered.contains(&expected), "{rendered}");
}

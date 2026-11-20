//! Thread tool contracts are table-driven.
#![allow(clippy::too_many_lines)]

use std::io::Cursor;

use agent::{Tool, ToolContext, ToolError, ToolOutput, ToolRequest};
use image::{DynamicImage, ImageFormat};
use runtime::{Address, DeliveryTarget, MemoryStore, Prompt, Store};
use serde_json::{Value, json};

use crate::fake_source::{Fake, file};
use crate::{ReadThread, ReadThreadFile, ThreadHistory, ThreadMessage};

fn png() -> Vec<u8> {
    let mut bytes = Cursor::new(Vec::new());
    DynamicImage::new_luma8(2, 3)
        .write_to(&mut bytes, ImageFormat::Png)
        .unwrap();
    bytes.into_inner()
}

fn message(id: &str, text: &str, files: Vec<crate::FileRef>) -> ThreadMessage {
    ThreadMessage {
        id: id.to_owned(),
        author: "U2".to_owned(),
        own: false,
        text: text.to_owned(),
        files,
        reactions: Vec::new(),
    }
}

fn thread(messages: Vec<ThreadMessage>) -> ThreadHistory {
    ThreadHistory {
        messages,
        complete: true,
    }
}

/// An operation accepted in a routed thread, and the context a durable
/// driver hands its tool calls.
async fn operation(store: &MemoryStore, routed: bool) -> Option<ToolContext> {
    let address = Address::new("test", "C1", "172.000").unwrap();
    let mut prompt = Prompt::new(address, "Ev1", "system", "read it").unwrap();
    if routed {
        let target = DeliveryTarget::new("test", json!({"conversation": "C1"})).unwrap();
        prompt = prompt.with_delivery(target);
    }
    let id = store.accept(&prompt).await.unwrap().operation.id;
    Some(ToolContext {
        operation_id: u64::try_from(id.get()).unwrap(),
        identity: None,
        profile: agent::ExecutionProfile::default(),
    })
}

async fn call(
    tool: &impl Tool,
    args: Value,
    context: Option<ToolContext>,
) -> Result<ToolOutput, ToolError> {
    let request = ToolRequest {
        args,
        idempotency_key: Some("key".to_owned()),
        context,
    };
    tool.call(request).await
}

fn document(output: &ToolOutput) -> Value {
    serde_json::from_str(&output.text).unwrap()
}

#[tokio::test]
async fn read_thread_returns_messages_with_file_references_and_reactions() {
    let store = MemoryStore::default();
    let context = operation(&store, true).await;
    let shot = file("F1", "écran ☕.png", "image/png", 9);
    let mut seen = message("172.004", "see ☕", vec![shot]);
    seen.reactions = vec![crate::Reaction {
        name: "eyes".to_owned(),
        count: 2,
        own: true,
    }];
    let source = Fake {
        history: Some(thread(vec![seen])),
        ..Fake::default()
    };
    let tool = ReadThread::new(source, store);
    let output = call(&tool, json!({}), context).await.unwrap();
    assert_eq!(output.images, []);
    assert_eq!(
        document(&output),
        json!({"outcome": "thread_read", "complete": true, "more": false, "messages": [{
            "id": "172.004", "author": "U2", "own": false, "text": "see ☕",
            "files": [{"id": "F1", "name": "écran ☕.png", "type": "image/png", "size": 9}],
            "reactions": [{"name": "eyes", "count": 2, "own": true}]
        }]})
    );
}

#[tokio::test]
async fn read_thread_pages_backwards_from_a_message_id() {
    let store = MemoryStore::default();
    let context = operation(&store, true).await;
    let messages = (1..=5)
        .map(|index| message(&format!("172.00{index}"), "m", Vec::new()))
        .collect();
    let source = Fake {
        history: Some(thread(messages)),
        ..Fake::default()
    };
    let tool = ReadThread::new(source, store);
    let page = document(
        &call(&tool, json!({"limit": 2}), context.clone())
            .await
            .unwrap(),
    );
    let ids: Vec<&str> = page["messages"]
        .as_array()
        .unwrap()
        .iter()
        .map(|m| m["id"].as_str().unwrap())
        .collect();
    assert_eq!(ids, ["172.004", "172.005"]);
    assert_eq!(page["more"], true);
    let older = document(
        &call(
            &tool,
            json!({"before": "172.004", "limit": 5}),
            context.clone(),
        )
        .await
        .unwrap(),
    );
    assert_eq!(older["messages"].as_array().unwrap().len(), 3);
    assert_eq!(older["more"], false);
    let unknown = call(&tool, json!({"before": "999.000"}), context)
        .await
        .unwrap_err();
    assert_eq!(unknown.code, "unknown_thread_message");
}

#[tokio::test]
async fn read_thread_file_returns_an_image_as_model_input() {
    let store = MemoryStore::default();
    let context = operation(&store, true).await;
    let shot = file("F1", "écran ☕.png", "image/png", png().len());
    let mut source = Fake::serving(&[(&shot, &png())]);
    source.history = Some(thread(vec![message("172.004", "", vec![shot])]));
    let tool = ReadThreadFile::new(source, store);
    let output = call(&tool, json!({"file_id": "F1"}), context)
        .await
        .unwrap();
    assert_eq!(output.images.len(), 1);
    let attachment = media::attachment(&output.images[0]);
    assert_eq!(
        document(&output),
        json!({
            "outcome": "file_read", "id": "F1", "name": "écran ☕.png", "type": "image/png",
            "images": [attachment],
        })
    );
}

#[tokio::test]
async fn read_thread_file_returns_text_inline() {
    let store = MemoryStore::default();
    let context = operation(&store, true).await;
    let notes = file("F2", "notes.txt", "text/plain", 9);
    let mut source = Fake::serving(&[(&notes, "résumé ☕".as_bytes())]);
    source.history = Some(thread(vec![message("172.004", "", vec![notes])]));
    let tool = ReadThreadFile::new(source, store);
    let text = call(&tool, json!({"file_id": "F2"}), context)
        .await
        .unwrap();
    assert_eq!(document(&text)["content"], "résumé ☕");
    assert_eq!(text.images, []);
}

#[tokio::test]
async fn read_thread_file_reports_the_type_the_surface_resolved() {
    let store = MemoryStore::default();
    let context = operation(&store, true).await;
    let complete = file("F1", "écran.png", "image/png", png().len());
    let partial = crate::FileRef {
        media_type: None,
        size: None,
        url: None,
        ..complete.clone()
    };
    let mut source = Fake::serving(&[(&complete, &png())]);
    source.described.insert("F1".to_owned(), complete);
    source.history = Some(thread(vec![message("172.004", "", vec![partial])]));
    let tool = ReadThreadFile::new(source, store);
    let output = call(&tool, json!({"file_id": "F1"}), context)
        .await
        .unwrap();
    assert_eq!(document(&output)["type"], "image/png");
    assert_eq!(output.images.len(), 1);
}

// The trust boundary: a file id the model names is fetched only when it
// belongs to the thread this operation runs in.
#[tokio::test]
async fn a_file_outside_the_thread_is_refused_without_a_download() {
    let store = MemoryStore::default();
    let context = operation(&store, true).await;
    let elsewhere = file("F9", "secret.png", "image/png", 9);
    let mut source = Fake::serving(&[(&elsewhere, &png())]);
    source.history = Some(thread(vec![message(
        "172.004",
        "no files here",
        Vec::new(),
    )]));
    let downloads = std::sync::Arc::clone(&source.downloads);
    let tool = ReadThreadFile::new(source, store);
    let error = call(&tool, json!({"file_id": "F9"}), context)
        .await
        .unwrap_err();
    assert_eq!(error.code, "file_not_in_thread");
    assert!(downloads.lock().unwrap().is_empty());
}

#[tokio::test]
async fn a_file_the_model_cannot_read_reports_its_skip_code() {
    let store = MemoryStore::default();
    let context = operation(&store, true).await;
    let archive = file("F3", "data.zip", "application/zip", 9);
    let source = Fake {
        history: Some(thread(vec![message("172.004", "", vec![archive])])),
        ..Fake::default()
    };
    let tool = ReadThreadFile::new(source, store);
    let error = call(&tool, json!({"file_id": "F3"}), context)
        .await
        .unwrap_err();
    assert_eq!(error.code, "unsupported_attachment");
}

#[tokio::test]
async fn thread_tools_fail_with_stable_codes_outside_a_routed_thread() {
    let store = MemoryStore::default();
    let unrouted = operation(&store, false).await;
    let source = Fake {
        history: Some(thread(Vec::new())),
        ..Fake::default()
    };
    let tool = ReadThread::new(source, store);
    let error = call(&tool, json!({}), unrouted).await.unwrap_err();
    assert_eq!(error.code, "thread_unavailable");
    let error = call(&tool, json!({}), None).await.unwrap_err();
    assert_eq!(error.code, "durable_context_required");
    let error = call(&tool, json!({"limit": 0}), None).await.unwrap_err();
    assert_eq!(error.code, "invalid_thread_arguments");
}

#[tokio::test]
async fn a_surface_failure_keeps_the_surfaces_code() {
    let store = MemoryStore::default();
    let context = operation(&store, true).await;
    let tool = ReadThreadFile::new(Fake::default(), store);
    let error = call(&tool, json!({"file_id": "F1"}), context)
        .await
        .unwrap_err();
    assert_eq!(error.code, "surface_down");
    let bad = call(&tool, json!({"file": "F1"}), None).await.unwrap_err();
    assert_eq!(bad.code, "invalid_thread_file_arguments");
}

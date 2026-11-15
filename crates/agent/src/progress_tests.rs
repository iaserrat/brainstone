use crate::run_tests::Recorder;
use crate::{Event, Progress};

fn text(delta: &str) -> Event {
    Event::ModelDelta {
        delta: delta.into(),
    }
}

fn reasoning(delta: &str) -> Event {
    Event::ReasoningDelta {
        delta: delta.into(),
    }
}

#[tokio::test]
async fn interleaved_deltas_leave_in_arrival_order_with_same_kind_runs_merged() {
    let mut progress = Progress::default();
    let mut sink = Recorder::default();
    progress.reasoning("pensé ", &mut sink).await.unwrap();
    progress.reasoning("☕", &mut sink).await.unwrap();
    progress.text("answer ", &mut sink).await.unwrap();
    progress.reasoning("again", &mut sink).await.unwrap();
    progress.text("done", &mut sink).await.unwrap();
    progress.flush(&mut sink).await.unwrap();
    assert_eq!(
        sink.events,
        vec![
            reasoning("pensé ☕"),
            text("answer "),
            reasoning("again"),
            text("done")
        ]
    );
}

#[tokio::test]
async fn a_full_buffer_flushes_at_512_characters_not_bytes() {
    let mut progress = Progress::default();
    let mut sink = Recorder::default();
    progress.text(&"é".repeat(511), &mut sink).await.unwrap();
    assert_eq!(sink.events, Vec::<Event>::new());
    progress.text("é", &mut sink).await.unwrap();
    assert_eq!(sink.events, vec![text(&"é".repeat(512))]);
}

#[tokio::test]
async fn empty_deltas_and_empty_flushes_emit_nothing() {
    let mut progress = Progress::default();
    let mut sink = Recorder::default();
    progress.text("held", &mut sink).await.unwrap();
    progress.reasoning("", &mut sink).await.unwrap();
    assert_eq!(sink.events, Vec::<Event>::new());
    progress.flush(&mut sink).await.unwrap();
    progress.flush(&mut sink).await.unwrap();
    assert_eq!(sink.events, vec![text("held")]);
}

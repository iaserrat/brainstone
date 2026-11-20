// These shared behavior cases intentionally exceed the production function budget.
#![allow(clippy::too_many_lines)]
use agent::{Event, Message, Outcome};
use brainstone_runtime::{
    Address, AdmissionQuota, Admitted, DeliveryCheckpoint, DeliveryClaim, DeliveryStore,
    DeliveryTarget, DurableStore, InteractionRequest, InteractionStore, LaneStore, Memories,
    Memory, Prompt, ScheduleRequest, ScheduleStore, ScheduleTime, Store,
};
use serde_json::json;
use std::time::Duration;

pub(crate) async fn ancillary(store: &DurableStore, session: &str) {
    let address = Address::new("contract", session, "lane").unwrap();
    let target = DeliveryTarget::new(session, json!({"thread":"one"})).unwrap();
    let prompt = Prompt::new(address.clone(), "one", "system", "héllo ☕")
        .unwrap()
        .with_delivery(target.clone())
        .with_actor(session);
    let id = store.accept(&prompt).await.unwrap().operation.id;
    assert!(store.request_exists(&address, "one").await.unwrap());
    assert_eq!(store.actor_operations_last_day(session).await.unwrap(), 1);
    let interaction = InteractionRequest::new(
        id,
        format!("{session}/question"),
        json!({"question":"continue?"}),
    )
    .unwrap();
    let opened = store.open_interaction(&interaction).await.unwrap();
    store
        .mark_interaction_delivered(&interaction.id, "posted")
        .await
        .unwrap();
    store
        .answer_interaction(&interaction.id, &json!({"yes":true}))
        .await
        .unwrap();
    assert_eq!(
        store
            .interaction(&interaction.id)
            .await
            .unwrap()
            .unwrap()
            .response,
        Some(json!({"yes":true}))
    );
    assert_eq!(opened.target, target);
    assert_eq!(
        opened.client_message_id.len(),
        36,
        "surface receipts use UUIDs"
    );
    assert_eq!(opened.client_message_id.as_bytes()[14], b'4');
    let memory = Memory::new(session, &format!("{session}/memory"), "Résumé %_ ☕").unwrap();
    store.save_memory(&memory).await.unwrap();
    store.save_memory(&memory).await.unwrap();
    assert_eq!(
        store
            .recall_memories(session, Some("résumé %_"), 10)
            .await
            .unwrap(),
        vec!["Résumé %_ ☕"]
    );
    store
        .unfollow_lane(id, &format!("{session}/stop"))
        .await
        .unwrap();
    assert!(!store.lane_is_followed(&address).await.unwrap());
    store.follow_lane(&address, "mention").await.unwrap();
    store
        .unfollow_lane(id, &format!("{session}/stop"))
        .await
        .unwrap();
    assert!(store.lane_is_followed(&address).await.unwrap());
    let claimed = store
        .claim_operation(id, "worker", Duration::from_secs(30))
        .await
        .unwrap()
        .unwrap();
    let steering = Prompt::new(address, "two", "new system", "steer").unwrap();
    assert!(matches!(
        store.admit_with_tools(&steering, &[]).await.unwrap(),
        Admitted::Steering { .. }
    ));
    assert_eq!(
        store.steering(&claimed.lease).await.unwrap(),
        vec![Message::User("steer".to_owned())]
    );
    let media = brainstone_runtime::Media::Image(
        agent::Image::new(agent::ImageMediaType::Png, vec![1, 2, 3], 1, 1).unwrap(),
    );
    store
        .save_media(&claimed.lease, std::slice::from_ref(&media))
        .await
        .unwrap();
    assert_eq!(
        store.media(id, &media.reference()).await.unwrap(),
        Some(media)
    );
    store
        .settle(
            &claimed.lease,
            &Outcome::Completed {
                text: "done".to_owned(),
            },
        )
        .await
        .unwrap();
    let claim = DeliveryClaim::new(session, "surface", Duration::from_secs(30), 100).unwrap();
    let delivery = store.claim_delivery(&claim).await.unwrap().unwrap();
    assert_eq!(delivery.target, target);
    assert!(
        delivery
            .events
            .iter()
            .any(|event| matches!(event.event, Event::RunFinished { .. }))
    );
    store
        .save_delivery_stream(&delivery.lease, Some("posted"))
        .await
        .unwrap();
    store
        .renew_delivery(&delivery.lease, Duration::from_secs(30))
        .await
        .unwrap();
    let checkpoint = DeliveryCheckpoint::new(
        delivery.lease,
        delivery.events.last().unwrap().sequence,
        Some("posted".to_owned()),
    );
    store.checkpoint_delivery(&checkpoint).await.unwrap();
    assert!(store.claim_delivery(&claim).await.unwrap().is_none());
}

pub(crate) async fn schedules(store: &DurableStore, session: &str) {
    let address = Address::new("schedule", session, "lane").unwrap();
    let prompt = Prompt::new(address, "one", "system", "schedule").unwrap();
    let id = store.accept(&prompt).await.unwrap().operation.id;
    let request =
        ScheduleRequest::new(id, "due", "follow up", ScheduleTime::AfterSeconds(0)).unwrap();
    let scheduled = store.schedule(&request).await.unwrap();
    assert_eq!(store.schedule(&request).await.unwrap(), scheduled);
    assert_eq!(store.schedules(id).await.unwrap(), vec![scheduled]);
    finish(store, id).await;
    let due = store
        .claim("scheduler", Duration::from_secs(30))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(due.thread.last(), &Message::User("follow up".to_owned()));
    store
        .settle(
            &due.lease,
            &Outcome::Completed {
                text: "done".to_owned(),
            },
        )
        .await
        .unwrap();
    let request =
        ScheduleRequest::new(id, "later", "cancel", ScheduleTime::AfterSeconds(3600)).unwrap();
    let scheduled = store.schedule(&request).await.unwrap();
    let cancelled = store.cancel_schedule(id, scheduled.id).await.unwrap();
    assert_eq!(
        cancelled.state,
        brainstone_runtime::ScheduleState::Cancelled
    );
    assert_eq!(
        store.cancel_schedule(id, scheduled.id).await.unwrap(),
        cancelled
    );
}

pub(crate) async fn finish(store: &impl Store, id: brainstone_runtime::OperationId) {
    let claimed = store
        .claim_operation(id, "finish", Duration::from_secs(30))
        .await
        .unwrap()
        .unwrap();
    store
        .settle(
            &claimed.lease,
            &Outcome::Completed {
                text: "done".to_owned(),
            },
        )
        .await
        .unwrap();
}

pub(crate) async fn posts(store: &DurableStore, session: &str) {
    let identity = agent::ExecutionIdentity {
        provider: "test".to_owned(),
        tenant: session.to_owned(),
        actor: "owner".to_owned(),
        conversation: "direct".to_owned(),
        private: true,
    };
    let address = Address::new("post", &identity.conversation_key(), "origin").unwrap();
    let target = DeliveryTarget::new("posts", json!({"channel":"origin"})).unwrap();
    let prompt = Prompt::new(address, "one", "system", "post elsewhere")
        .unwrap()
        .with_identity(Some(identity.clone()))
        .with_delivery(target);
    let parent = store.accept(&prompt).await.unwrap().operation.id;
    let destination_identity = agent::ExecutionIdentity {
        conversation: "channel".to_owned(),
        private: false,
        ..identity
    };
    let destination = brainstone_runtime::Destination {
        surface: "post".to_owned(),
        conversation: destination_identity.conversation_key(),
        target: DeliveryTarget::new("posts", json!({"channel":"destination"})).unwrap(),
        identity: destination_identity.clone(),
    };
    let request = ScheduleRequest::new(
        parent,
        "post",
        "hello channel",
        ScheduleTime::AfterSeconds(0),
    )
    .unwrap()
    .with_destination(destination);
    let scheduled = store.schedule(&request).await.unwrap();
    finish(store, parent).await;
    let released = store
        .claim("scheduler", Duration::from_secs(30))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(released.identity, Some(destination_identity.clone()));
    assert_eq!(
        released.thread.last(),
        &Message::User("hello channel".to_owned())
    );
    store
        .settle(
            &released.lease,
            &Outcome::Completed {
                text: "posted".to_owned(),
            },
        )
        .await
        .unwrap();
    let claim = DeliveryClaim::new("posts", "delivery", Duration::from_secs(30), 100).unwrap();
    let first = store.claim_delivery(&claim).await.unwrap().unwrap();
    store
        .checkpoint_delivery(&DeliveryCheckpoint::new(
            first.lease,
            first.events.last().unwrap().sequence,
            None,
        ))
        .await
        .unwrap();
    let post = store.claim_delivery(&claim).await.unwrap().unwrap();
    assert_eq!(post.lease.operation(), released.operation.id);
    let reply =
        DeliveryTarget::new("posts", json!({"channel":"destination","thread":"root"})).unwrap();
    assert!(store.bind_lane(&post.lease, "root", &reply).await.unwrap());
    assert_eq!(
        store.delivery_target(released.operation.id).await.unwrap(),
        Some(reply)
    );
    let response = Prompt::new(
        Address::new("post", &destination_identity.conversation_key(), "root").unwrap(),
        "reply",
        "system",
        "continue",
    )
    .unwrap()
    .with_identity(Some(destination_identity));
    let next = store.admit_with_tools(&response, &[]).await.unwrap();
    let Admitted::Operation(next) = next else {
        panic!("new operation")
    };
    assert_eq!(next.operation.lane_id, released.operation.lane_id);
    assert_eq!(
        store
            .cancel_schedule(parent, scheduled.id)
            .await
            .unwrap_err()
            .code(),
        "schedule_not_found"
    );
    finish(store, next.operation.id).await;
}

// Recovery scenarios retain their complete durable boundaries.
#![allow(clippy::too_many_lines)]
use agent::{Event, ExecutionIdentity, Message, Outcome};
use brainstone_runtime::{
    Address, Admitted, Compaction, DeliveryClaim, DeliveryStore, DeliveryTarget, DurableStore,
    InteractionRequest, InteractionStore, LaneStore, Memories, Memory, Prompt, PromptContent,
    ScheduleRequest, ScheduleStore, ScheduleTime, Store, SubagentConfig, SubagentRequest,
    SubagentStore,
};
use serde_json::json;
use std::time::Duration;

pub(crate) async fn check(url: &str) {
    let store = DurableStore::connect(url).await.unwrap();
    let identity = ExecutionIdentity {
        provider: "restart".to_owned(),
        tenant: "contract".to_owned(),
        actor: "alice".to_owned(),
        conversation: "direct".to_owned(),
        private: true,
    };
    let address = Address::new("restart", &identity.conversation_key(), "lane").unwrap();
    let image = agent::Image::new(agent::ImageMediaType::Png, vec![1, 2, 3], 2, 3).unwrap();
    let content = PromptContent {
        text: "read".to_owned(),
        images: vec![image.clone()],
    };
    let target = DeliveryTarget::new("restart", json!({"thread":"root"})).unwrap();
    let prompt = Prompt::with_content(address.clone(), "one", "system", content)
        .unwrap()
        .with_identity(Some(identity.clone()))
        .with_delivery(target);
    let accepted = store.accept(&prompt).await.unwrap();
    let id = accepted.operation.id;
    let lease = store
        .claim_operation(id, "before", Duration::from_secs(1))
        .await
        .unwrap()
        .unwrap()
        .lease;
    store.record(&lease, &Event::RunStarted).await.unwrap();
    let media = brainstone_runtime::Media::Image(image.clone());
    store
        .save_media(&lease, std::slice::from_ref(&media))
        .await
        .unwrap();
    let memory = Memory::new("restart", "restart-memory", "remember ☕").unwrap();
    store.save_memory(&memory).await.unwrap();
    let question =
        InteractionRequest::new(id, "restart-question", json!({"question":"continue?"})).unwrap();
    store.open_interaction(&question).await.unwrap();
    store
        .answer_interaction(&question.id, &json!({"yes":true}))
        .await
        .unwrap();
    let request =
        ScheduleRequest::new(id, "later", "follow up", ScheduleTime::AfterSeconds(3600)).unwrap();
    let schedule = store.schedule(&request).await.unwrap();
    let tools = vec![
        json!({"name":"lookup","description":"Look up a record","input_schema":{"type":"object"}}),
    ];
    let child = SubagentConfig::new("child", "child system", agent::ExecutionProfile::default())
        .unwrap()
        .with_tools(tools)
        .in_background(true);
    let spawn = SubagentRequest::new(id, "child", "task", child).unwrap();
    let child = store.spawn_subagent(&spawn).await.unwrap().agent;
    store.unfollow_lane(id, "restart-stop").await.unwrap();
    let steering = Prompt::new(address.clone(), "steer", "new system", "steering ☕")
        .unwrap()
        .with_identity(Some(identity.clone()));
    assert!(matches!(
        store.admit_with_tools(&steering, &[]).await.unwrap(),
        Admitted::Steering { .. }
    ));
    let delivery_claim =
        DeliveryClaim::new("restart", "surface-before", Duration::from_secs(1), 100).unwrap();
    let delivery = store
        .claim_delivery(&delivery_claim)
        .await
        .unwrap()
        .unwrap();
    store
        .save_delivery_stream(&delivery.lease, Some("posted-before-crash"))
        .await
        .unwrap();
    store.close().await;
    tokio::time::sleep(Duration::from_millis(1010)).await;

    let store = DurableStore::connect(url).await.unwrap();
    assert_eq!(
        store.recall_memories("restart", None, 10).await.unwrap(),
        vec!["remember ☕"]
    );
    assert_eq!(
        store
            .interaction(&question.id)
            .await
            .unwrap()
            .unwrap()
            .response,
        Some(json!({"yes":true}))
    );
    assert_eq!(store.schedules(id).await.unwrap()[0].id, schedule.id);
    assert_eq!(
        store.subagent(id, child.id).await.unwrap(),
        Some(child.clone())
    );
    assert!(!store.lane_is_followed(&address).await.unwrap());
    assert_eq!(
        store.media(id, &media.reference()).await.unwrap(),
        Some(media.clone())
    );
    let recovered = store
        .claim_operation(id, "after", Duration::from_secs(30))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(recovered.identity, Some(identity.clone()));
    assert_eq!(
        recovered.thread.messages(),
        &[Message::UserImages(
            agent::UserImages::new("read", vec![image]).unwrap()
        )]
    );
    assert_eq!(
        store.steering(&recovered.lease).await.unwrap(),
        vec![Message::User("steering ☕".to_owned())]
    );
    assert_eq!(
        store
            .claim_delivery(
                &DeliveryClaim::new("restart", "surface-after", Duration::from_secs(30), 100)
                    .unwrap()
            )
            .await
            .unwrap()
            .unwrap()
            .external_id,
        Some("posted-before-crash".to_owned())
    );
    store
        .settle(
            &recovered.lease,
            &Outcome::Completed {
                text: "done".to_owned(),
            },
        )
        .await
        .unwrap();
    let child_run = store
        .claim_operation(child.operation.id, "child-after", Duration::from_secs(30))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(child_run.identity, Some(identity.clone()));
    assert_eq!(child_run.thread.profile().max_turns(), None);
    assert_eq!(child_run.tool_grant, Some(vec!["lookup".to_owned()]));
    store
        .settle(
            &child_run.lease,
            &Outcome::Completed {
                text: "child done".to_owned(),
            },
        )
        .await
        .unwrap();
    let notification = store
        .claim("notify", Duration::from_secs(30))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(notification.identity, Some(identity.clone()));
    store
        .settle(
            &notification.lease,
            &Outcome::Completed {
                text: "noted".to_owned(),
            },
        )
        .await
        .unwrap();
    store
        .compact(&Compaction::new(
            accepted.operation.lane_id,
            id,
            "durable summary",
        ))
        .await
        .unwrap();
    store.close().await;
    let store = DurableStore::connect(url).await.unwrap();
    let next = Prompt::new(address, "next", "system", "continue")
        .unwrap()
        .with_identity(Some(identity));
    let next = store.accept(&next).await.unwrap().operation.id;
    let claimed = store
        .claim_operation(next, "compacted", Duration::from_secs(30))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        claimed.thread.messages().first(),
        Some(&Message::Summary("durable summary".to_owned()))
    );
    assert_eq!(
        store.media(next, &media.reference()).await.unwrap(),
        Some(media)
    );
    store
        .settle(
            &claimed.lease,
            &Outcome::Completed {
                text: "finished".to_owned(),
            },
        )
        .await
        .unwrap();
    store.close().await;
}

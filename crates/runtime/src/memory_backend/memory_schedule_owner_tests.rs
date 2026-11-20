//! Who may see and cancel a schedule, and how many one owner may hold, through
//! the public store seams.

use agent::ExecutionIdentity;
use jiff::civil::Time;

use crate::schedule_owner::MAX_REPEAT_PER_OWNER;
use crate::{
    Address, Error, MemoryStore, OperationId, Prompt, Recurrence, ScheduleRequest, ScheduleState,
    ScheduleStore, ScheduleTime, Store,
};

fn identity(actor: &str, conversation: &str, private: bool) -> ExecutionIdentity {
    ExecutionIdentity {
        provider: "test".into(),
        tenant: "T1".into(),
        actor: actor.into(),
        conversation: conversation.into(),
        private,
    }
}

/// An accepted operation in `lane` of `identity`'s conversation.
async fn caller(store: &MemoryStore, identity: ExecutionIdentity, lane: &str) -> OperationId {
    let address = Address::new("test", &identity.conversation_key(), lane).unwrap();
    let prompt = Prompt::new(
        address,
        &format!("{lane}-{}", identity.actor),
        "system",
        "hi",
    )
    .unwrap()
    .with_identity(Some(identity));
    store.accept(&prompt).await.unwrap().operation.id
}

fn daily() -> ScheduleTime {
    ScheduleTime::Repeat(Recurrence::new(Time::constant(9, 0, 0, 0), "UTC", None).unwrap())
}

#[tokio::test]
async fn an_owner_sees_and_cancels_their_schedules_from_their_direct_messages() {
    let store = MemoryStore::default();
    let thread = caller(&store, identity("U1", "C1", false), "t1").await;
    let request = ScheduleRequest::new(thread, "standup", "post the standup", daily()).unwrap();
    let id = store.schedule(&request).await.unwrap().id;

    let own_dm = caller(&store, identity("U1", "D1", true), "d1").await;
    let listed = store.schedules(own_dm).await.unwrap();
    assert_eq!(listed.len(), 1, "{listed:?}");
    assert_eq!(listed[0].owner.as_deref(), Some("U1"));

    let other_dm = caller(&store, identity("U2", "D2", true), "d1").await;
    assert_eq!(store.schedules(other_dm).await.unwrap(), []);
    assert_eq!(
        store.cancel_schedule(other_dm, id).await,
        Err(Error::ScheduleNotFound)
    );
    let other_thread = caller(&store, identity("U1", "C1", false), "t2").await;
    assert!(
        store.schedules(other_thread).await.unwrap().is_empty(),
        "owner-wide listing is only for the owner's private conversations"
    );

    let cancelled = store.cancel_schedule(own_dm, id).await.unwrap();
    assert_eq!(cancelled.state, ScheduleState::Cancelled);
    assert_eq!(store.schedules(thread).await.unwrap(), []);
}

#[tokio::test]
async fn one_owner_holds_a_bounded_number_of_standing_schedules_across_lanes() {
    let store = MemoryStore::default();
    let per_lane = usize::try_from(crate::MAX_REPEAT_PER_LANE).unwrap();
    let owner_cap = usize::try_from(MAX_REPEAT_PER_OWNER).unwrap();
    for index in 0..owner_cap {
        let lane = format!("d{}", index / per_lane);
        let origin = caller(&store, identity("U1", "D1", true), &lane).await;
        let request = ScheduleRequest::new(origin, &format!("k{index}"), "x", daily()).unwrap();
        store.schedule(&request).await.unwrap();
    }
    let fresh = caller(&store, identity("U1", "D1", true), "fresh").await;
    let extra = ScheduleRequest::new(fresh, "extra", "x", daily()).unwrap();
    assert_eq!(
        store.schedule(&extra).await,
        Err(Error::ScheduleOwnerLimitExceeded)
    );
    let other = caller(&store, identity("U2", "D2", true), "d0").await;
    let theirs = ScheduleRequest::new(other, "theirs", "x", daily()).unwrap();
    assert!(
        store.schedule(&theirs).await.is_ok(),
        "the cap is per owner"
    );
}

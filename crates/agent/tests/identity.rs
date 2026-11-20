// Table-driven missing-fact cases: restrictive access fails closed.
#![allow(clippy::too_many_lines)]
use brainstone_agent::{AccessPolicy, ExecutionIdentity};

fn identity() -> ExecutionIdentity {
    ExecutionIdentity {
        provider: "test".into(),
        tenant: "T1".into(),
        actor: "U1".into(),
        conversation: "D1".into(),
        private: true,
    }
}

#[test]
fn restrictive_access_fails_closed() {
    let policy = AccessPolicy {
        authenticated_actor: true,
        private_conversation: true,
    };
    assert_eq!(
        policy.check(None).unwrap_err().code(),
        "authenticated_actor_required"
    );
    for field in ["actor", "tenant", "provider", "conversation"] {
        let mut identity = identity();
        match field {
            "actor" => identity.actor = String::new(),
            "tenant" => identity.tenant = "  ".into(),
            "provider" => identity.provider = String::new(),
            _ => identity.conversation = String::new(),
        }
        assert_eq!(
            policy.check(Some(&identity)).unwrap_err().code(),
            "authenticated_actor_required"
        );
    }
    let mut shared = identity();
    shared.private = false;
    assert_eq!(
        policy.check(Some(&shared)).unwrap_err().code(),
        "private_conversation_required"
    );
    assert!(policy.check(Some(&identity())).is_ok());
    assert!(AccessPolicy::default().check(None).is_ok());
}

#[test]
fn tenant_and_private_owner_are_part_of_history_identity() {
    let first = identity();
    let mut second = first.clone();
    second.tenant = "T2".into();
    assert_ne!(first.scope(), second.scope());
    assert_ne!(first.conversation_key(), second.conversation_key());
    second = first.clone();
    second.actor = "U2".into();
    assert_ne!(first.conversation_key(), second.conversation_key());
}

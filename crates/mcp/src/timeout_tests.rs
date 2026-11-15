use std::time::Duration;

#[tokio::test(start_paused = true)]
async fn interaction_time_is_paused_without_resetting_the_active_budget() {
    let (waiting, receiver) = tokio::sync::watch::channel(false);
    let call = async move {
        tokio::time::sleep(Duration::from_secs(4)).await;
        waiting.send(true).unwrap();
        tokio::time::sleep(Duration::from_secs(100)).await;
        waiting.send(false).unwrap();
        tokio::time::sleep(Duration::from_secs(7)).await;
    };

    let result = crate::timeout::unless_waiting(call, Duration::from_secs(10), receiver).await;

    assert_eq!(result, Err(()));
}

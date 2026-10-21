// SPDX-License-Identifier: MIT
//! Dropping a timed-out provider future prevents any late result reaching insertion.
use std::{future::Future, time::Duration};
use tokio_util::sync::CancellationToken;
pub const AUDIO_TIMEOUT: &str = "audio was unable to process, please try again.";
pub async fn run<T, F: Future<Output = Result<T, String>>>(
    duration: Duration,
    parent: &CancellationToken,
    work: impl FnOnce(CancellationToken) -> F,
) -> Result<T, String> {
    let child = parent.child_token();
    let result = tokio::select! {
        biased;
        _ = parent.cancelled() => Err("Cancelled".into()),
        result = tokio::time::timeout(duration, work(child.clone())) => result.unwrap_or_else(|_| Err(AUDIO_TIMEOUT.into())),
    };
    child.cancel();
    result
}
#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    };
    #[tokio::test]
    async fn timeout_cancels_request_and_drops_late_result() {
        let parent = CancellationToken::new();
        let completed = Arc::new(AtomicBool::new(false));
        let flag = completed.clone();
        let observed = Arc::new(Mutex::new(None));
        let capture = observed.clone();
        let result = run(Duration::from_millis(15), &parent, |token| async move {
            *capture.lock().unwrap() = Some(token);
            tokio::time::sleep(Duration::from_millis(80)).await;
            flag.store(true, Ordering::SeqCst);
            Ok(())
        })
        .await;
        assert_eq!(result.unwrap_err(), AUDIO_TIMEOUT);
        assert!(!parent.is_cancelled());
        assert!(observed.lock().unwrap().as_ref().unwrap().is_cancelled());
        tokio::time::sleep(Duration::from_millis(90)).await;
        assert!(!completed.load(Ordering::SeqCst));
    }
    #[tokio::test]
    async fn cancellation_wins_over_ready_result_and_fast_results_survive() {
        let parent = CancellationToken::new();
        assert_eq!(
            run(Duration::from_secs(1), &parent, |_| async { Ok(42) })
                .await
                .unwrap(),
            42
        );
        parent.cancel();
        assert_eq!(
            run(Duration::from_secs(1), &parent, |_| async { Ok(42) })
                .await
                .unwrap_err(),
            "Cancelled"
        );
    }
}

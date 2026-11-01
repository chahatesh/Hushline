// SPDX-License-Identifier: MIT
//! Retry only work known not to have changed the receiving field. A dispatched
//! edit, including an uncertain/partial dispatch, must never be replayed.
use std::time::{Duration, Instant};
use tokio_util::sync::CancellationToken;

pub const RETRY_WINDOW: Duration = Duration::from_secs(5);
pub const RETRY_INTERVAL: Duration = Duration::from_millis(100);
// The worker owns the five-second budget. Allow its result to reach the actor
// before the outer cancellation fence; restoration continues independently.
pub const DELIVERY_TIMEOUT: Duration = Duration::from_secs(6);

#[derive(Debug)]
pub enum Failure {
    Retry(String),
    Stop(String),
}
impl From<String> for Failure {
    fn from(message: String) -> Self {
        Self::Stop(message)
    }
}
impl From<&str> for Failure {
    fn from(message: &str) -> Self {
        Self::Stop(message.into())
    }
}
pub type Attempt<T> = Result<T, Failure>;

pub struct Budget {
    started: Instant,
}
impl Default for Budget {
    fn default() -> Self {
        Self::new()
    }
}
impl Budget {
    pub fn new() -> Self {
        Self {
            started: Instant::now(),
        }
    }
    pub fn remaining(&self) -> Duration {
        RETRY_WINDOW.saturating_sub(self.started.elapsed())
    }
    pub fn check(&self, cancel: &CancellationToken) -> Result<(), String> {
        if cancel.is_cancelled() {
            Err("Cancelled".into())
        } else if self.remaining().is_zero() {
            Err("Paste could not finish within five seconds. Your text is preserved.".into())
        } else {
            Ok(())
        }
    }
    pub fn run<T>(
        &self,
        cancel: &CancellationToken,
        mut attempt: impl FnMut() -> Attempt<T>,
    ) -> Result<T, String> {
        retry(
            cancel,
            || self.started.elapsed(),
            |duration| {
                // Bound cancellation response even between attempts.
                let until = Instant::now() + duration;
                while !cancel.is_cancelled() && Instant::now() < until {
                    std::thread::sleep(
                        until
                            .saturating_duration_since(Instant::now())
                            .min(Duration::from_millis(10)),
                    );
                }
            },
            &mut attempt,
        )
    }
}

fn retry<T>(
    cancel: &CancellationToken,
    elapsed: impl Fn() -> Duration,
    mut sleep: impl FnMut(Duration),
    mut attempt: impl FnMut() -> Attempt<T>,
) -> Result<T, String> {
    let mut last_error =
        "Paste could not finish within five seconds. Your text is preserved.".into();
    loop {
        if cancel.is_cancelled() {
            return Err("Cancelled".into());
        }
        let remaining = RETRY_WINDOW.saturating_sub(elapsed());
        if remaining.is_zero() {
            return Err(last_error);
        }
        match attempt() {
            Ok(result) => return Ok(result),
            Err(Failure::Stop(error)) => return Err(error),
            Err(Failure::Retry(error)) => last_error = error,
        }
        let remaining = RETRY_WINDOW.saturating_sub(elapsed());
        if !remaining.is_zero() {
            sleep(RETRY_INTERVAL.min(remaining));
        }
    }
}

/// SendInput reports accepted input events, not a receipt from the app. Only
/// zero is safe to replay; partial dispatch could already include Ctrl+V.
pub fn input_dispatch(accepted: u32) -> Attempt<()> {
    match accepted {
        4 => Ok(()),
        0 => Err(Failure::Retry(
            "Windows is blocking paste. Retrying for up to five seconds; your text is preserved.".into(),
        )),
        _ => Err(Failure::Stop(
            "Windows accepted only part of the paste shortcut. Check the field before trying again; your text is preserved.".into(),
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::{Cell, RefCell};

    #[test]
    fn retries_every_hundred_ms_for_five_seconds_then_stops() {
        let elapsed = Cell::new(Duration::ZERO);
        let attempts = RefCell::new(Vec::new());
        let result: Result<(), String> = retry(
            &CancellationToken::new(),
            || elapsed.get(),
            |pause| elapsed.set(elapsed.get() + pause),
            || {
                attempts.borrow_mut().push(elapsed.get());
                Err(Failure::Retry("Clipboard busy".into()))
            },
        );
        assert_eq!(result.unwrap_err(), "Clipboard busy");
        assert_eq!(elapsed.get(), RETRY_WINDOW);
        assert_eq!(attempts.borrow().len(), 50);
        assert!(attempts
            .borrow()
            .windows(2)
            .all(|w| w[1] - w[0] == RETRY_INTERVAL));
    }

    #[test]
    fn slow_attempts_share_the_same_deadline_and_cannot_extend_it() {
        let elapsed = Cell::new(Duration::ZERO);
        let attempts = Cell::new(0);
        let _: Result<(), String> = retry(
            &CancellationToken::new(),
            || elapsed.get(),
            |pause| elapsed.set(elapsed.get() + pause),
            || {
                attempts.set(attempts.get() + 1);
                elapsed.set(elapsed.get() + Duration::from_millis(1150));
                Err(Failure::Retry("Field unavailable".into()))
            },
        );
        assert_eq!(attempts.get(), 4);
        assert_eq!(elapsed.get(), RETRY_WINDOW);
    }

    #[test]
    fn accepted_or_uncertain_paste_is_never_replayed() {
        for accepted in 0..=4 {
            let elapsed = Cell::new(Duration::ZERO);
            let calls = Cell::new(0);
            let result = retry(
                &CancellationToken::new(),
                || elapsed.get(),
                |pause| elapsed.set(elapsed.get() + pause),
                || {
                    calls.set(calls.get() + 1);
                    input_dispatch(accepted)
                },
            );
            assert_eq!(result.is_ok(), accepted == 4);
            assert_eq!(calls.get(), if accepted == 0 { 50 } else { 1 });
        }
    }

    #[test]
    fn cancelled_or_expired_work_cannot_reach_dispatch() {
        let cancel = CancellationToken::new();
        cancel.cancel();
        let result: Result<(), String> = retry(
            &cancel,
            || Duration::ZERO,
            |_| panic!("Cancelled work must not wait"),
            || panic!("Cancelled work must not dispatch"),
        );
        assert_eq!(result.unwrap_err(), "Cancelled");
        let expired = Budget {
            started: Instant::now() - RETRY_WINDOW,
        };
        assert!(expired.check(&CancellationToken::new()).is_err());
        assert!(expired
            .run(&CancellationToken::new(), || -> Attempt<()> {
                panic!("An expired worker must not dispatch")
            })
            .is_err());
    }

    #[test]
    fn transient_failures_recover_and_stop_after_one_success() {
        let elapsed = Cell::new(Duration::ZERO);
        let calls = Cell::new(0);
        let result = retry(
            &CancellationToken::new(),
            || elapsed.get(),
            |pause| elapsed.set(elapsed.get() + pause),
            || {
                calls.set(calls.get() + 1);
                if calls.get() < 4 {
                    Err(Failure::Retry("Waiting for field".into()))
                } else {
                    Ok("sent once")
                }
            },
        );
        assert_eq!(result.unwrap(), "sent once");
        assert_eq!(calls.get(), 4);
        assert_eq!(elapsed.get(), Duration::from_millis(300));
    }

    #[test]
    fn cancellation_and_permanent_failures_stop_before_another_attempt() {
        for cancel_during_wait in [false, true] {
            let cancel = CancellationToken::new();
            let calls = Cell::new(0);
            let result: Result<(), String> = retry(
                &cancel,
                || Duration::ZERO,
                |_| cancel.cancel(),
                || {
                    calls.set(calls.get() + 1);
                    if cancel_during_wait {
                        Err(Failure::Retry("Waiting".into()))
                    } else {
                        Err(Failure::Stop("Protected field".into()))
                    }
                },
            );
            assert_eq!(calls.get(), 1);
            assert_eq!(
                result.unwrap_err(),
                if cancel_during_wait {
                    "Cancelled"
                } else {
                    "Protected field"
                }
            );
        }
    }
}

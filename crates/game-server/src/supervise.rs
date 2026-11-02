//! Surviving a panic (`docs/server/audit.md` R5): a panic in one connection or background task must not stop the rest of the server.

use std::future::Future;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::task::Poll;
use std::time::Duration;

use tokio::task::JoinHandle;

/// How long a task that panicked waits before it starts again.
pub const RESTART_WAIT: Duration = Duration::from_secs(1);

/// The text of a panic, as far as it is one.
fn message(payload: &(dyn std::any::Any + Send)) -> String {
    payload
        .downcast_ref::<&str>()
        .map(|s| (*s).to_owned())
        .or_else(|| payload.downcast_ref::<String>().cloned())
        .unwrap_or_else(|| "a panic".into())
}

/// Runs `future`; a panic inside it comes back as an `Err` with its text instead of unwinding into the caller. The future is dropped
/// by then, and whatever it held (locks included) is released as in any unwind.
pub async fn catch<F: Future>(future: F) -> Result<F::Output, String> {
    let mut future = Box::pin(future);
    std::future::poll_fn(move |cx| match catch_unwind(AssertUnwindSafe(|| future.as_mut().poll(cx))) {
        Ok(Poll::Pending) => Poll::Pending,
        Ok(Poll::Ready(output)) => Poll::Ready(Ok(output)),
        Err(payload) => Poll::Ready(Err(message(&*payload))),
    })
    .await
}

/// The waits before the three more tries of a queued database write that failed (`docs/server/audit.md` R16).
pub const WRITE_RETRIES: [Duration; 3] = [Duration::from_millis(200), Duration::from_secs(1), Duration::from_secs(5)];

/// Runs `attempt` (a database write); while it fails with an error it waits `delays[n]` and runs it again. The last outcome comes
/// back: `Ok(Err(..))` when every try failed, `Err(panic)` when one panicked (a panic is not tried again).
pub async fn retried<F, Fut>(delays: &[Duration], mut attempt: F) -> Result<anyhow::Result<()>, String>
where
    F: FnMut() -> Fut,
    Fut: Future<Output = anyhow::Result<()>>,
{
    let mut tried = 0;
    loop {
        match catch(attempt()).await {
            Ok(Err(error)) if tried < delays.len() => {
                tracing::warn!(%error, tried = tried + 1, "a queued database write failed; trying again");
                tokio::time::sleep(delays[tried]).await;
                tried += 1;
            }
            outcome => return outcome,
        }
    }
}

/// A background task that starts again after a panic: logged, a second's wait, and a new run from `make`. A task that returns ends
/// the supervision. The task runs on a task of its own, so its panic is a `JoinError` here.
pub fn supervise<F, Fut>(name: &'static str, mut make: F) -> JoinHandle<()>
where
    F: FnMut() -> Fut + Send + 'static,
    Fut: Future<Output = ()> + Send + 'static,
{
    tokio::spawn(async move {
        loop {
            match tokio::spawn(make()).await {
                Ok(()) => return,
                Err(error) if error.is_panic() => {
                    tracing::error!(task = name, panic = %message(&*error.into_panic()), "a background task panicked; starting it again");
                    tokio::time::sleep(RESTART_WAIT).await;
                }
                // Cancelled with the runtime.
                Err(_) => return,
            }
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};

    #[tokio::test]
    async fn a_panic_in_a_future_comes_back_as_an_error() {
        assert_eq!(catch(async { 5 }).await, Ok(5));
        let caught = catch(async {
            if true {
                panic!("inside {}", "the future");
            }
        })
        .await;
        assert_eq!(caught, Err("inside the future".to_owned()));
    }

    #[tokio::test]
    async fn a_failed_write_is_tried_again_after_each_wait_and_then_given_up() {
        let tries = AtomicUsize::new(0);
        let delays = [Duration::from_millis(1); 3];
        // Fails twice, then succeeds.
        let outcome = retried(&delays, || async {
            if tries.fetch_add(1, Ordering::Relaxed) < 2 {
                anyhow::bail!("down");
            }
            Ok(())
        })
        .await;
        assert!(matches!(outcome, Ok(Ok(()))));
        assert_eq!(tries.load(Ordering::Relaxed), 3);
        // Always fails: the first try and three more, then the error comes back.
        tries.store(0, Ordering::Relaxed);
        let outcome = retried(&delays, || async {
            tries.fetch_add(1, Ordering::Relaxed);
            anyhow::bail!("still down")
        })
        .await;
        assert!(matches!(outcome, Ok(Err(_))));
        assert_eq!(tries.load(Ordering::Relaxed), 4);
        // A panic is not tried again.
        tries.store(0, Ordering::Relaxed);
        let outcome = retried(&delays, || async {
            tries.fetch_add(1, Ordering::Relaxed);
            if true {
                panic!("write bug");
            }
            Ok(())
        })
        .await;
        assert_eq!(outcome.unwrap_err(), "write bug");
        assert_eq!(tries.load(Ordering::Relaxed), 1);
    }

    #[tokio::test]
    async fn a_task_that_panics_starts_again_and_one_that_returns_ends() {
        let runs = Arc::new(AtomicUsize::new(0));
        let counted = runs.clone();
        let handle = supervise("test", move || {
            let runs = counted.clone();
            async move {
                // Panics on the first two runs, finishes on the third.
                if runs.fetch_add(1, Ordering::Relaxed) < 2 {
                    panic!("task bug");
                }
            }
        });
        tokio::time::timeout(Duration::from_secs(10), handle).await.expect("supervision ended").unwrap();
        assert_eq!(runs.load(Ordering::Relaxed), 3);
    }
}

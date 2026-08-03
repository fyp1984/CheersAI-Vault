use std::collections::VecDeque;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// Injectable clock so rate-limit tests never need a real `sleep`. `Instant`
/// is monotonic and cannot be manipulated by system-clock changes, matching
/// the "no real multi-minute wait in tests" requirement.
pub trait Clock: Send + Sync {
    fn now(&self) -> Instant;
}

/// Production clock: the real, monotonic wall clock.
#[derive(Default)]
pub struct SystemClock;

impl Clock for SystemClock {
    fn now(&self) -> Instant {
        Instant::now()
    }
}

/// Returned by [`RateLimiter::check`] when currently blocked.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RateLimited {
    pub retry_after: Duration,
}

struct State {
    /// Timestamps of failures still inside the observation window, oldest
    /// first.
    failures: VecDeque<Instant>,
    blocked_until: Option<Instant>,
}

/// Global (not per-client, not per-IP) sliding-window rate limiter:
/// `max_failures` failed verification attempts inside any `window` triggers
/// a `block` block on *all* further attempts, regardless of which caller
/// made them — this cannot be bypassed by switching browser or client. A
/// single successful verification clears the failure history immediately.
pub struct RateLimiter {
    clock: Arc<dyn Clock>,
    window: Duration,
    max_failures: u32,
    block: Duration,
    state: Mutex<State>,
}

impl RateLimiter {
    pub fn new(clock: Arc<dyn Clock>, window: Duration, max_failures: u32, block: Duration) -> Self {
        Self {
            clock,
            window,
            max_failures,
            block,
            state: Mutex::new(State {
                failures: VecDeque::new(),
                blocked_until: None,
            }),
        }
    }

    /// The fixed production policy: 5 failures inside any 5-minute window
    /// blocks all further attempts for 5 minutes.
    pub fn with_system_clock() -> Self {
        Self::new(
            Arc::new(SystemClock),
            Duration::from_secs(5 * 60),
            5,
            Duration::from_secs(5 * 60),
        )
    }

    /// Call before attempting a verification. Returns `Err` (with the
    /// remaining block duration) if currently blocked; the caller must not
    /// touch the PIN backend in that case — this is what keeps Argon2
    /// verification from running while blocked. Does not itself count as an
    /// attempt and never records a failure.
    pub fn check(&self) -> Result<(), RateLimited> {
        let now = self.clock.now();
        let mut state = self.state.lock().expect("rate limiter mutex poisoned");
        if let Some(until) = state.blocked_until {
            if now < until {
                return Err(RateLimited {
                    retry_after: until - now,
                });
            }
            // Block window elapsed: start counting fresh rather than
            // instantly re-blocking on stale pre-block failures.
            state.blocked_until = None;
            state.failures.clear();
        }
        Ok(())
    }

    /// Record a failed verification. If this failure is the `max_failures`th
    /// one inside the observation window, starts a new `block` block.
    pub fn record_failure(&self) {
        let now = self.clock.now();
        let mut state = self.state.lock().expect("rate limiter mutex poisoned");
        state.failures.push_back(now);
        let window = self.window;
        while let Some(&front) = state.failures.front() {
            if now.duration_since(front) > window {
                state.failures.pop_front();
            } else {
                break;
            }
        }
        if state.failures.len() as u32 >= self.max_failures {
            state.blocked_until = Some(now + self.block);
        }
    }

    /// Record a successful verification: clears the failure history so a
    /// correct PIN always resets the counter, even mid-window.
    pub fn record_success(&self) {
        let mut state = self.state.lock().expect("rate limiter mutex poisoned");
        state.failures.clear();
        state.blocked_until = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex as StdMutex;

    /// Deterministic, manually-advanced clock for tests. Starts at an
    /// arbitrary fixed instant and only moves forward when the test calls
    /// [`FakeClock::advance`] — no real time ever passes.
    struct FakeClock {
        now: StdMutex<Instant>,
    }

    impl FakeClock {
        fn new() -> Arc<Self> {
            Arc::new(Self {
                now: StdMutex::new(Instant::now()),
            })
        }
        fn advance(&self, d: Duration) {
            let mut now = self.now.lock().unwrap();
            *now += d;
        }
    }

    impl Clock for FakeClock {
        fn now(&self) -> Instant {
            *self.now.lock().unwrap()
        }
    }

    fn limiter(clock: Arc<FakeClock>) -> RateLimiter {
        RateLimiter::new(
            clock,
            Duration::from_secs(300),
            5,
            Duration::from_secs(300),
        )
    }

    #[test]
    fn allows_up_to_four_failures_without_blocking() {
        let clock = FakeClock::new();
        let rl = limiter(clock);
        for _ in 0..4 {
            assert!(rl.check().is_ok());
            rl.record_failure();
        }
        assert!(rl.check().is_ok(), "4 failures inside the window must not block yet");
    }

    #[test]
    fn fifth_failure_inside_window_blocks_further_attempts() {
        let clock = FakeClock::new();
        let rl = limiter(clock);
        for _ in 0..5 {
            assert!(rl.check().is_ok());
            rl.record_failure();
        }
        let err = rl.check().unwrap_err();
        assert_eq!(err.retry_after, Duration::from_secs(300));
    }

    #[test]
    fn block_recovers_after_the_full_duration_elapses() {
        let clock = FakeClock::new();
        let rl = limiter(clock.clone());
        for _ in 0..5 {
            rl.record_failure();
        }
        assert!(rl.check().is_err());
        clock.advance(Duration::from_secs(299));
        assert!(rl.check().is_err(), "must still be blocked 1 second before the block elapses");
        clock.advance(Duration::from_secs(2));
        assert!(rl.check().is_ok(), "must be unblocked once the full duration has elapsed");
    }

    #[test]
    fn success_clears_the_failure_counter() {
        let clock = FakeClock::new();
        let rl = limiter(clock);
        for _ in 0..4 {
            rl.record_failure();
        }
        rl.record_success();
        // Counter reset: 4 more failures (8 total, but only 4 since reset)
        // must still not trigger a block.
        for _ in 0..4 {
            assert!(rl.check().is_ok());
            rl.record_failure();
        }
        assert!(rl.check().is_ok());
    }

    #[test]
    fn failures_outside_the_observation_window_are_not_counted() {
        let clock = FakeClock::new();
        let rl = limiter(clock.clone());
        for _ in 0..4 {
            rl.record_failure();
        }
        // Push the clock past the 5-minute window so the first 4 failures
        // age out before the 5th one is recorded.
        clock.advance(Duration::from_secs(301));
        assert!(rl.check().is_ok());
        rl.record_failure();
        assert!(
            rl.check().is_ok(),
            "a single fresh failure after the old ones expired must not trigger a block"
        );
    }

    #[test]
    fn blocked_attempts_are_rejected_without_being_recorded_as_new_failures() {
        let clock = FakeClock::new();
        let rl = limiter(clock.clone());
        for _ in 0..5 {
            rl.record_failure();
        }
        let first_retry_after = rl.check().unwrap_err().retry_after;
        // Hammering `check()` while blocked must not extend the block.
        for _ in 0..10 {
            let retry_after = rl.check().unwrap_err().retry_after;
            assert!(retry_after <= first_retry_after);
        }
    }

    #[test]
    fn concurrent_failures_are_counted_atomically_without_races() {
        use std::thread;

        let clock = FakeClock::new();
        let rl = Arc::new(limiter(clock));
        let mut handles = Vec::new();
        for _ in 0..5 {
            let rl = Arc::clone(&rl);
            handles.push(thread::spawn(move || {
                rl.record_failure();
            }));
        }
        for h in handles {
            h.join().unwrap();
        }
        assert!(
            rl.check().is_err(),
            "5 concurrently recorded failures must still trigger exactly one block, not be lost to a race"
        );
    }
}

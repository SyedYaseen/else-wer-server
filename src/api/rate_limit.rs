use std::{
    collections::HashMap,
    sync::Mutex,
    time::{Duration, Instant},
};

use crate::api::api_error::ApiError;

const FAILURE_WINDOW: Duration = Duration::from_secs(15 * 60);
const MAX_FAILURES_BEFORE_LOCKOUT: u32 = 5;
const BASE_LOCKOUT: Duration = Duration::from_secs(15 * 60);
const MAX_LOCKOUT: Duration = Duration::from_secs(60 * 60);

struct Attempt {
    failures: u32,
    window_start: Instant,
    locked_until: Option<Instant>,
}

/// Per-(ip, username) login attempt tracker. After `MAX_FAILURES_BEFORE_LOCKOUT`
/// failures within `FAILURE_WINDOW`, the key is locked out for `BASE_LOCKOUT`,
/// doubling on each further failure while still locked (capped at `MAX_LOCKOUT`).
/// Purely in-memory: resets on restart, which is fine for a single-instance
/// self-hosted deployment.
pub struct LoginRateLimiter {
    attempts: Mutex<HashMap<String, Attempt>>,
}

impl LoginRateLimiter {
    pub fn new() -> Self {
        Self {
            attempts: Mutex::new(HashMap::new()),
        }
    }

    /// Returns `Err(ApiError::TooManyRequests)` if this key is currently locked out.
    pub fn check(&self, key: &str) -> Result<(), ApiError> {
        let attempts = self.attempts.lock().unwrap();
        let locked = attempts
            .get(key)
            .and_then(|attempt| attempt.locked_until)
            .is_some_and(|until| Instant::now() < until);

        if locked {
            return Err(ApiError::TooManyRequests(
                "Too many failed login attempts. Try again later.".into(),
            ));
        }
        Ok(())
    }

    pub fn record_failure(&self, key: &str) {
        let mut attempts = self.attempts.lock().unwrap();
        let now = Instant::now();
        let attempt = attempts.entry(key.to_string()).or_insert_with(|| Attempt {
            failures: 0,
            window_start: now,
            locked_until: None,
        });

        if now.duration_since(attempt.window_start) > FAILURE_WINDOW && attempt.locked_until.is_none()
        {
            attempt.failures = 0;
            attempt.window_start = now;
        }

        attempt.failures += 1;

        if attempt.failures >= MAX_FAILURES_BEFORE_LOCKOUT {
            let prior_lockouts = attempt.failures - MAX_FAILURES_BEFORE_LOCKOUT;
            let lockout = BASE_LOCKOUT
                .saturating_mul(1u32 << prior_lockouts.min(2))
                .min(MAX_LOCKOUT);
            attempt.locked_until = Some(now + lockout);
        }
    }

    pub fn record_success(&self, key: &str) {
        let mut attempts = self.attempts.lock().unwrap();
        attempts.remove(key);
    }
}

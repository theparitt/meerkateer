use std::{
    collections::HashMap,
    net::IpAddr,
    time::{Duration, Instant},
};

use tokio::sync::Mutex;

const WINDOW: Duration = Duration::from_secs(60);
const IDLE_TTL: Duration = Duration::from_secs(10 * 60);
const MAX_BUCKETS: usize = 65_536;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum Scope {
    Authentication,
    InvitationAcceptance,
    PasswordLogin,
    GameProbe,
    Ingestion,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct Key {
    scope: Scope,
    address: IpAddr,
}

#[derive(Debug)]
struct Bucket {
    tokens: f64,
    updated_at: Instant,
}

#[derive(Debug)]
struct LimiterState {
    buckets: HashMap<Key, Bucket>,
    last_sweep: Instant,
}

#[derive(Debug)]
pub(crate) struct RateLimiter {
    state: Mutex<LimiterState>,
}

impl RateLimiter {
    pub(crate) fn new() -> Self {
        Self {
            state: Mutex::new(LimiterState {
                buckets: HashMap::new(),
                last_sweep: Instant::now(),
            }),
        }
    }

    pub(crate) async fn allow(&self, scope: Scope, address: IpAddr, limit: u32) -> bool {
        let now = Instant::now();
        let mut state = self.state.lock().await;
        if now.duration_since(state.last_sweep) >= WINDOW {
            state
                .buckets
                .retain(|_, bucket| now.duration_since(bucket.updated_at) < IDLE_TTL);
            state.last_sweep = now;
        }

        let key = Key { scope, address };
        if !state.buckets.contains_key(&key) && state.buckets.len() >= MAX_BUCKETS {
            return false;
        }
        let capacity = f64::from(limit);
        let bucket = state.buckets.entry(key).or_insert(Bucket {
            tokens: capacity,
            updated_at: now,
        });
        let elapsed = now.duration_since(bucket.updated_at).as_secs_f64();
        let refill_per_second = capacity / WINDOW.as_secs_f64();
        bucket.tokens = (bucket.tokens + elapsed * refill_per_second).min(capacity);
        bucket.updated_at = now;
        if bucket.tokens < 1.0 {
            return false;
        }
        bucket.tokens -= 1.0;
        true
    }
}

#[cfg(test)]
mod tests {
    use std::net::{IpAddr, Ipv4Addr};

    use super::{RateLimiter, Scope};

    #[tokio::test]
    async fn independent_scopes_are_bounded_per_peer() {
        let limiter = RateLimiter::new();
        let peer = IpAddr::V4(Ipv4Addr::LOCALHOST);
        assert!(limiter.allow(Scope::Authentication, peer, 2).await);
        assert!(limiter.allow(Scope::Authentication, peer, 2).await);
        assert!(!limiter.allow(Scope::Authentication, peer, 2).await);
        assert!(limiter.allow(Scope::PasswordLogin, peer, 1).await);
        assert!(!limiter.allow(Scope::PasswordLogin, peer, 1).await);
        assert!(limiter.allow(Scope::Ingestion, peer, 1).await);
        assert!(!limiter.allow(Scope::Ingestion, peer, 1).await);
    }

    #[tokio::test]
    async fn peers_do_not_consume_each_others_capacity() {
        let limiter = RateLimiter::new();
        let first = IpAddr::V4(Ipv4Addr::new(192, 0, 2, 1));
        let second = IpAddr::V4(Ipv4Addr::new(192, 0, 2, 2));
        assert!(limiter.allow(Scope::Authentication, first, 1).await);
        assert!(!limiter.allow(Scope::Authentication, first, 1).await);
        assert!(limiter.allow(Scope::Authentication, second, 1).await);
    }
}

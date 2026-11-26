//! Sign-in protection: too many wrong passwords from one address, or for one account, and
//! further attempts are refused for a while.
//!
//! Kept in memory. A restart forgets it, which is acceptable: the point is to make guessing slow,
//! and Argon2 already makes each guess expensive.

use std::collections::{HashMap, VecDeque};
use std::net::SocketAddr;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use axum::extract::ConnectInfo;
use axum::http::HeaderMap;

/// Failures are counted over this window.
pub const WINDOW: Duration = Duration::from_secs(15 * 60);
/// Wrong passwords from one address (any accounts) before it must wait.
pub const PER_ADDRESS: usize = 20;
/// Wrong passwords for one account (from anywhere) before it must wait.
pub const PER_ACCOUNT: usize = 8;
/// Above this many tracked keys, old ones are dropped (keeps memory bounded under a flood).
const MAX_KEYS: usize = 100_000;

#[derive(Default)]
pub struct SignInGuard {
    failures: Mutex<HashMap<String, VecDeque<Instant>>>,
}

impl SignInGuard {
    /// How long `key` must wait, if it has used up its attempts.
    pub fn blocked(&self, key: &str, max: usize) -> Option<Duration> {
        let mut map = self.failures.lock().unwrap();
        let now = Instant::now();
        let list = map.get_mut(key)?;
        while list.front().is_some_and(|t| now.duration_since(*t) >= WINDOW) {
            list.pop_front();
        }
        if list.len() >= max {
            let oldest = *list.front()?;
            Some(WINDOW.saturating_sub(now.duration_since(oldest)))
        } else {
            None
        }
    }

    /// Record a wrong password. Returns how many failures `key` now has in the window.
    pub fn fail(&self, key: &str) -> usize {
        let mut map = self.failures.lock().unwrap();
        let now = Instant::now();
        if map.len() >= MAX_KEYS {
            map.retain(|_, l| l.back().is_some_and(|t| now.duration_since(*t) < WINDOW));
            if map.len() >= MAX_KEYS {
                map.clear();
            }
        }
        let list = map.entry(key.to_string()).or_default();
        while list.front().is_some_and(|t| now.duration_since(*t) >= WINDOW) {
            list.pop_front();
        }
        list.push_back(now);
        list.len()
    }

    pub fn clear(&self, key: &str) {
        self.failures.lock().unwrap().remove(key);
    }
}

/// The address a request came from. Behind a proxy (Caddy, a Cloudflare tunnel) every request
/// arrives from the proxy, so the real address is read from the header the proxy sets, named in
/// `MP_CLIENT_IP_HEADER` (for example `X-Forwarded-For` or `CF-Connecting-IP`).
pub fn client_ip(header: Option<&str>, headers: &HeaderMap, conn: Option<&ConnectInfo<SocketAddr>>) -> String {
    if let Some(name) = header {
        if let Some(v) = headers.get(name).and_then(|v| v.to_str().ok()) {
            // X-Forwarded-For may list several; the first is the original client.
            if let Some(first) = v.split(',').next().map(str::trim).filter(|s| !s.is_empty()) {
                return first.to_string();
            }
        }
    }
    conn.map(|c| c.0.ip().to_string()).unwrap_or_else(|| "unknown".into())
}

pub fn minutes(d: Duration) -> u64 {
    d.as_secs().div_ceil(60).max(1)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn blocks_after_max_and_clears() {
        let g = SignInGuard::default();
        for _ in 0..PER_ACCOUNT {
            assert!(g.blocked("a", PER_ACCOUNT).is_none());
            g.fail("a");
        }
        let wait = g.blocked("a", PER_ACCOUNT).expect("blocked");
        assert!(wait <= WINDOW && wait > Duration::from_secs(14 * 60));
        assert!(g.blocked("b", PER_ACCOUNT).is_none());
        g.clear("a");
        assert!(g.blocked("a", PER_ACCOUNT).is_none());
    }

    #[test]
    fn client_ip_prefers_configured_header() {
        let mut h = HeaderMap::new();
        h.insert("x-forwarded-for", "203.0.113.7, 10.0.0.1".parse().unwrap());
        let conn = ConnectInfo("172.18.0.5:5000".parse::<SocketAddr>().unwrap());
        assert_eq!(client_ip(Some("X-Forwarded-For"), &h, Some(&conn)), "203.0.113.7");
        assert_eq!(client_ip(None, &h, Some(&conn)), "172.18.0.5");
        assert_eq!(client_ip(Some("CF-Connecting-IP"), &h, Some(&conn)), "172.18.0.5");
    }
}

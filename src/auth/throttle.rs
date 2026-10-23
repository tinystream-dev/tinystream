// SPDX-License-Identifier: AGPL-3.0-or-later

use std::collections::HashMap;
use std::net::{IpAddr, Ipv6Addr, SocketAddr};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use axum::extract::{ConnectInfo, Request};

use crate::config::Network;

/// Wrong passwords an address gets before it has to wait.
const BURST: f64 = 10.0;

/// How long it waits for each one after that.
const EVERY: Duration = Duration::from_secs(30);

/// Keeps anyone from guessing passwords quickly, once Funnel or a reverse proxy puts the sign-in
/// page on the internet.
#[derive(Default)]
pub struct Throttle(Mutex<HashMap<IpAddr, (f64, Instant)>>);

impl Throttle {
    /// Spends one of `ip`'s guesses, or says how long until it has one. Guesses that turn out right
    /// are given back with [`Throttle::right`].
    pub fn guess(&self, ip: Option<IpAddr>) -> Result<(), String> {
        let Some(ip) = ip.map(key) else { return Ok(()) };
        let mut map = self.0.lock().unwrap();

        if map.len() > 10_000 {
            map.retain(|_, (left, at)| refill(*left, *at) < BURST);
        }

        let (left, at) = map.entry(ip).or_insert((BURST, Instant::now()));
        let now = refill(*left, *at);

        if now < 1.0 {
            let wait = EVERY.mul_f64(1.0 - now).as_secs() + 1;
            return Err(format!("too many wrong passwords; try again in {wait} seconds"));
        }

        (*left, *at) = (now - 1.0, Instant::now());

        if *left < 1.0 {
            tracing::warn!("{ip} got {BURST} passwords wrong; it can only try once every {}s now", EVERY.as_secs());
        }

        Ok(())
    }

    pub fn right(&self, ip: Option<IpAddr>) {
        let Some(ip) = ip.map(key) else { return };

        if let Some((left, at)) = self.0.lock().unwrap().get_mut(&ip) {
            (*left, *at) = ((refill(*left, *at) + 1.0).min(BURST), Instant::now());
        }
    }
}

fn refill(left: f64, at: Instant) -> f64 {
    (left + at.elapsed().as_secs_f64() / EVERY.as_secs_f64()).min(BURST)
}

/// Anyone with an IPv6 address usually has the whole /64 around it.
fn key(ip: IpAddr) -> IpAddr {
    match ip {
        IpAddr::V6(v6) => match v6.to_ipv4_mapped() {
            Some(v4) => IpAddr::V4(v4),
            None => IpAddr::V6(Ipv6Addr::from(u128::from(v6) & !(u64::MAX as u128))),
        },
        ip => ip,
    }
}

/// Who's asking: the connection's address, or the one a proxy it trusts (like Tailscale Serve and
/// Funnel) says it's passing along.
pub fn client(req: &Request, network: &Network) -> Option<IpAddr> {
    let peer = req.extensions().get::<ConnectInfo<SocketAddr>>()?.0.ip();

    if network.trusts_proxy(peer) {
        let forwarded = req.headers().get("x-forwarded-for").and_then(|v| v.to_str().ok());

        if let Some(ip) = forwarded.and_then(|v| v.rsplit(',').next()).and_then(|v| v.trim().parse().ok()) {
            return Some(ip);
        }

        if let Some(ip) =
            req.headers().get("x-real-ip").and_then(|v| v.to_str().ok()).and_then(|v| v.trim().parse().ok())
        {
            return Some(ip);
        }
    }

    Some(peer)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wrong_guesses_run_out_and_right_ones_dont() {
        let throttle = Throttle::default();
        let ip = Some("203.0.113.7".parse().unwrap());

        for _ in 0..100 {
            throttle.guess(ip).unwrap();
            throttle.right(ip);
        }

        for _ in 0..BURST as usize {
            throttle.guess(ip).unwrap();
        }

        assert!(throttle.guess(ip).is_err());
        assert!(throttle.guess(Some("203.0.113.8".parse().unwrap())).is_ok());
        assert!(throttle.guess(None).is_ok());
    }

    #[test]
    fn ipv6_counts_by_64() {
        let throttle = Throttle::default();

        for i in 0..BURST as u16 {
            throttle.guess(Some(format!("2001:db8::{i:x}").parse().unwrap())).unwrap();
        }

        assert!(throttle.guess(Some("2001:db8::ffff:1".parse().unwrap())).is_err());
        assert!(throttle.guess(Some("2001:db8:0:1::1".parse().unwrap())).is_ok());
    }

    fn asks(peer: &str, forwarded: &str, network: &Network) -> Option<IpAddr> {
        let mut req = Request::builder().header("x-forwarded-for", forwarded).body(axum::body::Body::empty()).unwrap();
        req.extensions_mut().insert(ConnectInfo(SocketAddr::new(peer.parse().unwrap(), 1234)));
        client(&req, network)
    }

    #[test]
    fn only_trusted_proxies_pass_addresses_along() {
        let network = Network { trusted_proxies: vec!["172.16.0.0/12".into(), "fd00::1".into()], ..Network::default() };
        let forwarded = Some("203.0.113.7".parse().unwrap());

        assert_eq!(asks("127.0.0.1", "203.0.113.7", &network), forwarded);
        assert_eq!(asks("::ffff:127.0.0.1", "203.0.113.7", &network), forwarded);
        assert_eq!(asks("172.18.0.2", "10.0.0.1, 203.0.113.7", &network), forwarded);
        assert_eq!(asks("fd00::1", "203.0.113.7", &network), forwarded);
        assert_eq!(asks("192.168.1.20", "203.0.113.7", &network), Some("192.168.1.20".parse().unwrap()));
        assert_eq!(asks("fd00::2", "203.0.113.7", &network), Some("fd00::2".parse().unwrap()));
        assert_eq!(asks("172.18.0.2", "203.0.113.7", &Network::default()), Some("172.18.0.2".parse().unwrap()));
    }
}

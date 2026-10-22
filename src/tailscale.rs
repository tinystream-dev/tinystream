// SPDX-License-Identifier: AGPL-3.0-or-later

use std::collections::HashMap;
use std::net::IpAddr;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use serde::Deserialize;
use serde_json::{Map, Value, json};

use crate::config::{Config, Expose, FUNNEL_PORTS, Network};
use crate::paths::resolve_config_path;
use crate::state::AppState;

const SOCKETS: [&str; 2] = ["/var/run/tailscale/tailscaled.sock", "/run/tailscale/tailscaled.sock"];

/// How often to look again, for tailscaled starting, stopping or the machine being renamed.
const EVERY: Duration = Duration::from_secs(5 * 60);

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, async_graphql::Enum)]
#[graphql(name = "TailscaleState")]
pub enum State {
    /// `[tailscale] detect` is off.
    Off,

    #[default]
    Missing,

    /// There's a tailnet interface, but tailscaled's socket can't be reached.
    Unreachable,

    /// tailscaled is there but not connected to a tailnet.
    Stopped,

    Running,
}

#[derive(Debug, Clone, Default, PartialEq, async_graphql::SimpleObject)]
#[graphql(name = "TailscaleStatus")]
pub struct Status {
    pub state: State,
    pub dns_name: Option<String>,
    pub ips: Vec<String>,

    /// Tailscale Serve puts tinystream behind HTTPS.
    pub https: bool,

    /// Funnel opens that to the internet.
    pub funnel: bool,

    /// Where the tailnet reaches tinystream, when it can.
    pub url: Option<String>,
    pub warning: Option<String>,

    /// Why `[tailscale] expose` couldn't be set up.
    pub serve_error: Option<String>,
}

/// The Serve entry tinystream put in place, so it can take it away again.
#[derive(Debug, Clone, PartialEq)]
struct Entry {
    host_port: String,
    port: u16,
    proxy: String,
}

#[derive(Default)]
pub struct Tailscale {
    status: Mutex<Status>,
    entry: Mutex<Option<(PathBuf, Entry)>>,
    busy: tokio::sync::Mutex<()>,
}

impl Tailscale {
    pub fn url(&self) -> Option<String> {
        self.status.lock().unwrap().url.clone()
    }

    pub async fn refresh(&self, config: &Config, config_dir: &Path) -> Status {
        let _busy = self.busy.lock().await;
        let status = self.detect(config, config_dir).await;
        let mut current = self.status.lock().unwrap();

        if status.url != current.url {
            match &status.url {
                Some(url) => tracing::info!("on a tailnet; links point to {url}"),
                None if current.url.is_some() => tracing::info!("no longer reachable on the tailnet"),
                None => {},
            }
        }

        if status.serve_error != current.serve_error
            && let Some(e) = &status.serve_error
        {
            tracing::warn!("can't set up Tailscale Serve: {e}");
        }

        *current = status.clone();
        status
    }

    /// Takes tinystream's Serve entry away, for shutting down; it's put back on the way up.
    pub async fn withdraw(&self) {
        let _busy = self.busy.lock().await;
        self.remove_entry().await;
    }

    async fn remove_entry(&self) {
        let Some((socket, entry)) = self.entry.lock().unwrap().take() else { return };

        match remove(&socket, &entry).await {
            Ok(()) => tracing::info!("took tinystream off Tailscale Serve"),
            Err(e) => tracing::warn!("can't take tinystream off Tailscale Serve: {e}"),
        }
    }

    async fn detect(&self, config: &Config, config_dir: &Path) -> Status {
        let expose = config.tailscale.expose;

        if !config.tailscale.detect {
            self.remove_entry().await;
            return Status { state: State::Off, ..Status::default() };
        }

        let configured = config.tailscale.socket.as_deref().map(str::trim).filter(|s| !s.is_empty());

        let sockets: Vec<PathBuf> = match configured {
            Some(s) => resolve_config_path(s, config_dir).into_iter().collect(),
            None => SOCKETS.iter().map(PathBuf::from).collect(),
        };

        let mut failure = None;
        let mut found = None;

        for socket in &sockets {
            match local_api::<LocalStatus>(socket, "status").await {
                Ok(status) => {
                    found = Some((socket, status));
                    break;
                },
                Err(e) => failure = failure.or(Some((socket, e))),
            }
        }

        let unserved = |what: &str| {
            (expose != Expose::Manual).then(|| format!("{what}, so tinystream can't set up Tailscale Serve."))
        };

        let Some((socket, local)) = found else {
            if configured.is_none() && !has_interface() {
                return Status { serve_error: unserved("Tailscale isn't running here"), ..Status::default() };
            }

            let warning = match failure {
                Some((socket, e)) => format!(
                    "This machine looks like it's on a tailnet, but tinystream can't reach tailscaled at {} ({}). \
                     In Docker, mount /var/run/tailscale into the container.",
                    socket.display(),
                    e.root_cause()
                ),
                None => {
                    "This machine looks like it's on a tailnet, but tinystream can't find tailscaled's socket.".into()
                },
            };

            return Status {
                state: State::Unreachable,
                warning: Some(warning),
                serve_error: unserved("tailscaled can't be reached"),
                ..Status::default()
            };
        };

        let caps = local.caps();
        let me = local.me.unwrap_or_default();
        let dns_name = Some(me.dns_name.trim_end_matches('.').to_string()).filter(|n| !n.is_empty());
        let ips: Vec<String> = me.ips.iter().map(IpAddr::to_string).collect();

        if local.backend_state != "Running" {
            return Status {
                state: State::Stopped,
                dns_name,
                ips,
                warning: Some(format!("Tailscale is installed but isn't connected ({}).", local.backend_state)),
                serve_error: unserved("Tailscale isn't connected"),
                ..Status::default()
            };
        }

        let serve_error = match expose {
            Expose::Manual => {
                self.remove_entry().await;
                None
            },
            _ => self.apply(socket, &caps, dns_name.as_deref(), &me.ips, config).await.err(),
        };

        let served = match local_api::<Value>(socket, "serve-config").await {
            Ok(serve) => served_at(&serve, &config.network, &me.ips),
            Err(e) => {
                tracing::debug!("can't read Tailscale Serve's config: {e:#}");
                None
            },
        };

        if let Some((url, funnel)) = served {
            return Status {
                state: State::Running,
                dns_name,
                ips,
                https: true,
                funnel,
                url: Some(url),
                warning: None,
                serve_error,
            };
        }

        let magic = local.current_tailnet.is_some_and(|t| t.magic_dns);
        let host = dns_name.clone().filter(|_| magic).or_else(|| {
            let ip = me.ips.iter().find(|ip| ip.is_ipv4()).or(me.ips.first())?;
            Some(if ip.is_ipv6() { format!("[{ip}]") } else { ip.to_string() })
        });

        let (url, warning) = match (host, reachable(&config.network, &me.ips)) {
            (_, Err(e)) => (None, e),
            (None, Ok(())) => (None, "Tailscale is connected, but this machine has no tailnet address yet.".into()),
            (Some(host), Ok(())) => (
                Some(format!("http://{host}:{}", config.network.port)),
                "The tailnet reaches tinystream over plain HTTP, where browsers turn off passkeys and music playback. \
                 Let tinystream set up Tailscale Serve to put it behind HTTPS."
                    .into(),
            ),
        };

        Status { state: State::Running, dns_name, ips, url, warning: Some(warning), serve_error, ..Status::default() }
    }

    /// Makes Serve (and Funnel, for `public`) send `https-port` on this machine to tinystream.
    async fn apply(
        &self,
        socket: &Path,
        caps: &Caps,
        dns_name: Option<&str>,
        ips: &[IpAddr],
        config: &Config,
    ) -> Result<(), String> {
        let (port, public) = (config.tailscale.https_port, config.tailscale.expose == Expose::Public);
        let name = dns_name.ok_or("this machine has no tailnet name yet")?;

        if !caps.https {
            return Err("HTTPS certificates are off for this tailnet. Turn them on under DNS in the Tailscale admin \
                        console: https://login.tailscale.com/admin/dns"
                .into());
        }

        if public && !caps.funnel {
            return Err("Funnel isn't allowed for this machine. Add this to your tailnet's policy file \
                        (https://login.tailscale.com/admin/acls): \
                        \"nodeAttrs\": [{ \"target\": [\"autogroup:member\"], \"attr\": [\"funnel\"] }]"
                .into());
        }

        if public && !caps.funnel_ports.contains(&port) {
            let allowed: Vec<String> = caps.funnel_ports.iter().map(u16::to_string).collect();
            return Err(format!(
                "this tailnet only lets Funnel use port {}; set https-port to one of those",
                allowed.join(", ")
            ));
        }

        let entry = Entry { host_port: format!("{name}:{port}"), port, proxy: proxy_target(&config.network) };
        let previous = self.entry.lock().unwrap().as_ref().map(|(_, e)| e.clone());
        let (mut serve, etag) = read_serve(socket).await?;
        let before = serve.clone();

        if let Some(old) = previous.as_ref().filter(|old| **old != entry) {
            unset(&mut serve, old);
        }

        let ours = |proxy: &str| {
            is_ours(proxy, &config.network, ips) || previous.as_ref().is_some_and(|p| p.proxy == proxy.trim())
        };

        set(&mut serve, &entry, public, ours)?;

        if serve != before {
            write_serve(socket, &serve, &etag).await?;
            let url = https_url(&entry.host_port);

            match public {
                true => {
                    tracing::info!("Tailscale Serve sends {url} to tinystream, and Funnel opens it to the internet")
                },
                false => tracing::info!("Tailscale Serve sends {url} to tinystream"),
            }
        }

        *self.entry.lock().unwrap() = Some((socket.to_path_buf(), entry));
        Ok(())
    }
}

pub fn spawn(state: Arc<AppState>) {
    tokio::spawn(async move {
        let mut rx = state.config.subscribe();

        loop {
            let config = rx.borrow_and_update().clone();
            state.tailscale.refresh(&config, state.config.config_dir()).await;

            tokio::select! {
                r = rx.changed() => if r.is_err() { return },
                _ = tokio::time::sleep(EVERY) => {},
            }
        }
    });
}

#[derive(Deserialize)]
#[serde(rename_all = "PascalCase")]
struct LocalStatus {
    backend_state: String,
    #[serde(rename = "Self")]
    me: Option<Peer>,
    current_tailnet: Option<Tailnet>,
}

#[derive(Deserialize, Default)]
struct Peer {
    #[serde(rename = "DNSName", default)]
    dns_name: String,
    #[serde(rename = "TailscaleIPs", default)]
    ips: Vec<IpAddr>,
    #[serde(rename = "CapMap", default)]
    cap_map: Option<HashMap<String, Value>>,
    #[serde(rename = "Capabilities", default)]
    capabilities: Option<Vec<String>>,
}

#[derive(Deserialize)]
struct Tailnet {
    #[serde(rename = "MagicDNSEnabled", default)]
    magic_dns: bool,
}

/// What the tailnet's policy lets this machine do.
struct Caps {
    https: bool,
    funnel: bool,
    funnel_ports: Vec<u16>,
}

impl LocalStatus {
    fn caps(&self) -> Caps {
        let me = self.me.as_ref();
        let names: Vec<&str> = me
            .and_then(|m| m.cap_map.as_ref())
            .into_iter()
            .flat_map(|m| m.keys().map(String::as_str))
            .chain(me.and_then(|m| m.capabilities.as_ref()).into_iter().flatten().map(String::as_str))
            .collect();

        let funnel_ports = names
            .iter()
            .find_map(|c| c.strip_prefix("https://tailscale.com/cap/funnel-ports?ports="))
            .map(|ports| ports.split(',').filter_map(|p| p.trim().parse().ok()).collect())
            .unwrap_or_else(|| FUNNEL_PORTS.to_vec());

        Caps { https: names.contains(&"https"), funnel: names.contains(&"funnel"), funnel_ports }
    }
}

fn https_url(host_port: &str) -> String {
    match host_port.rsplit_once(':') {
        Some((host, "443")) => format!("https://{host}"),
        Some(_) => format!("https://{host_port}"),
        None => format!("https://{host_port}"),
    }
}

/// Where Serve should send requests to reach tinystream.
fn proxy_target(network: &Network) -> String {
    let host = match network.host.trim().parse::<IpAddr>() {
        Ok(ip) if ip.is_unspecified() => "127.0.0.1".to_string(),
        Ok(IpAddr::V6(ip)) => format!("[{ip}]"),
        Ok(ip) => ip.to_string(),
        Err(_) => network.host.trim().to_string(),
    };

    format!("http://{host}:{}", network.port)
}

/// Whether a Serve handler's proxy target is this tinystream.
fn is_ours(proxy: &str, network: &Network, ips: &[IpAddr]) -> bool {
    let proxy = proxy.trim();

    let url = match proxy.parse::<u16>() {
        Ok(port) => format!("http://127.0.0.1:{port}"),
        Err(_) if !proxy.contains("://") => format!("http://{proxy}"),
        Err(_) => proxy.to_string(),
    };

    let Ok(url) = url::Url::parse(&url) else { return false };

    let local = match url.host() {
        Some(url::Host::Domain(d)) => d == "localhost" || d == network.host,
        Some(url::Host::Ipv4(ip)) => ip.is_loopback() || ips.contains(&ip.into()) || ip.to_string() == network.host,
        Some(url::Host::Ipv6(ip)) => ip.is_loopback() || ips.contains(&ip.into()) || ip.to_string() == network.host,
        None => false,
    };

    url.scheme() == "http" && local && url.port_or_known_default() == Some(network.port)
}

/// The HTTPS address Tailscale Serve proxies to tinystream at, if it does, and whether Funnel opens
/// it.
fn served_at(serve: &Value, network: &Network, ips: &[IpAddr]) -> Option<(String, bool)> {
    for (host_port, web) in serve.get("Web").and_then(Value::as_object).into_iter().flatten() {
        let proxy = web.pointer("/Handlers/~1/Proxy").and_then(Value::as_str);

        if proxy.is_some_and(|p| is_ours(p, network, ips)) {
            let funnel = serve.get("AllowFunnel").and_then(|f| f.get(host_port)).and_then(Value::as_bool);
            return Some((https_url(host_port), funnel == Some(true)));
        }
    }

    serve.get("Foreground")?.as_object()?.values().find_map(|s| served_at(s, network, ips))
}

fn object<'a>(parent: &'a mut Value, key: &str) -> &'a mut Map<String, Value> {
    if !parent.is_object() {
        *parent = json!({});
    }

    let map = parent.as_object_mut().unwrap();
    let child = map.entry(key).or_insert_with(|| json!({}));

    if !child.is_object() {
        *child = json!({});
    }

    child.as_object_mut().unwrap()
}

/// Puts `entry` into Serve's config, unless something that isn't tinystream already has its place.
fn set(serve: &mut Value, entry: &Entry, funnel: bool, ours: impl Fn(&str) -> bool) -> Result<(), String> {
    let port = entry.port.to_string();
    let url = https_url(&entry.host_port);

    if let Some(tcp) = serve.pointer(&format!("/TCP/{port}"))
        && tcp.get("HTTPS").and_then(Value::as_bool) != Some(true)
    {
        return Err(format!(
            "Tailscale Serve already forwards port {port} somewhere else; set https-port to another port"
        ));
    }

    if let Some(handler) = serve.pointer(&format!("/Web/{}/Handlers/~1", entry.host_port)) {
        let proxy = handler.get("Proxy").and_then(Value::as_str);

        if !proxy.is_some_and(&ours) {
            return Err(format!(
                "Tailscale Serve already sends {url} to {}; set https-port to another port, or free it with \
                 `tailscale serve --https={port} off`",
                proxy.unwrap_or("something else")
            ));
        }
    }

    object(serve, "TCP").insert(port, json!({ "HTTPS": true }));

    let web = object(serve, "Web");
    let site = web.entry(entry.host_port.clone()).or_insert_with(|| json!({}));
    object(site, "Handlers").insert("/".into(), json!({ "Proxy": entry.proxy }));

    match funnel {
        true => {
            object(serve, "AllowFunnel").insert(entry.host_port.clone(), json!(true));
        },
        false => {
            object(serve, "AllowFunnel").remove(&entry.host_port);
        },
    }

    tidy(serve);
    Ok(())
}

/// Takes `entry` out of Serve's config, if it's still there as tinystream left it.
fn unset(serve: &mut Value, entry: &Entry) {
    let path = format!("/Web/{}/Handlers/~1/Proxy", entry.host_port);

    if serve.pointer(&path).and_then(Value::as_str) != Some(entry.proxy.as_str()) {
        return;
    }

    let web = object(serve, "Web");
    let site = web.get_mut(&entry.host_port).unwrap();
    object(site, "Handlers").remove("/");

    if site.get("Handlers").and_then(Value::as_object).is_some_and(Map::is_empty) {
        web.remove(&entry.host_port);
    }

    let suffix = format!(":{}", entry.port);

    if !web.keys().any(|k| k.ends_with(&suffix)) {
        object(serve, "TCP").remove(&entry.port.to_string());
    }

    object(serve, "AllowFunnel").remove(&entry.host_port);
    tidy(serve);
}

fn tidy(serve: &mut Value) {
    if let Some(map) = serve.as_object_mut() {
        map.retain(|_, v| !v.as_object().is_some_and(Map::is_empty));
    }
}

async fn remove(socket: &Path, entry: &Entry) -> Result<(), String> {
    let (mut serve, etag) = read_serve(socket).await?;
    let before = serve.clone();
    unset(&mut serve, entry);

    match serve != before {
        true => write_serve(socket, &serve, &etag).await,
        false => Ok(()),
    }
}

fn has_interface() -> bool {
    std::fs::read_dir("/sys/class/net")
        .map(|d| d.flatten().any(|e| e.file_name().to_string_lossy().starts_with("tailscale")))
        .unwrap_or(false)
}

fn reachable(network: &Network, ips: &[IpAddr]) -> Result<(), String> {
    let loopback = "tinystream only listens on localhost, so the rest of the tailnet can't reach it. \
                    Set [network] host to 0.0.0.0, or let tinystream set up Tailscale Serve.";

    match network.host.trim().parse::<IpAddr>() {
        Ok(ip) if ip.is_unspecified() || ips.contains(&ip) => Ok(()),
        Ok(ip) if ip.is_loopback() => Err(loopback.into()),
        Ok(ip) => Err(format!(
            "tinystream only listens on {ip}, which isn't this machine's tailnet address, so the tailnet can't reach it."
        )),
        Err(_) if network.host.trim() == "localhost" => Err(loopback.into()),
        Err(_) => Ok(()),
    }
}

#[cfg(unix)]
fn client(socket: &Path) -> anyhow::Result<reqwest::Client> {
    Ok(reqwest::Client::builder().unix_socket(socket.to_path_buf()).timeout(Duration::from_secs(5)).build()?)
}

fn endpoint(name: &str) -> String {
    format!("http://local-tailscaled.sock/localapi/v0/{name}")
}

#[cfg(unix)]
async fn local_api<T: serde::de::DeserializeOwned>(socket: &Path, name: &str) -> anyhow::Result<T> {
    Ok(client(socket)?.get(endpoint(name)).send().await?.error_for_status()?.json().await?)
}

#[cfg(not(unix))]
async fn local_api<T>(_: &Path, _: &str) -> anyhow::Result<T> {
    anyhow::bail!("tailscaled's socket can only be reached on Linux")
}

/// Serve's config, and the tag that makes sure nobody changed it in between when writing it back.
#[cfg(unix)]
async fn read_serve(socket: &Path) -> Result<(Value, String), String> {
    let read = async {
        let response = client(socket)?.get(endpoint("serve-config")).send().await?.error_for_status()?;
        let etag = response.headers().get("etag").and_then(|v| v.to_str().ok()).unwrap_or_default().to_string();
        anyhow::Ok((response.json::<Value>().await?, etag))
    };

    read.await.map_err(|e| format!("can't read Tailscale Serve's config: {}", e.root_cause()))
}

#[cfg(unix)]
async fn write_serve(socket: &Path, serve: &Value, etag: &str) -> Result<(), String> {
    let request =
        client(socket).map_err(|e| e.to_string())?.post(endpoint("serve-config")).header("If-Match", etag).json(serve);

    let response = request.send().await.map_err(|e| format!("can't reach tailscaled: {e}"))?;

    match response.status() {
        s if s.is_success() => Ok(()),
        reqwest::StatusCode::FORBIDDEN => {
            let user = std::env::var("USER").unwrap_or_else(|_| "<the user tinystream runs as>".into());
            Err(format!(
                "tinystream isn't allowed to change Tailscale's settings. Run `sudo tailscale set --operator={user}` once."
            ))
        },
        reqwest::StatusCode::PRECONDITION_FAILED => {
            Err("Tailscale Serve's config changed while tinystream was editing it; it'll try again shortly".into())
        },
        s => Err(format!("tailscaled said {s}: {}", response.text().await.unwrap_or_default().trim())),
    }
}

#[cfg(not(unix))]
async fn read_serve(_: &Path) -> Result<(Value, String), String> {
    Err("tailscaled's socket can only be reached on Linux".into())
}

#[cfg(not(unix))]
async fn write_serve(_: &Path, _: &Value, _: &str) -> Result<(), String> {
    Err("tailscaled's socket can only be reached on Linux".into())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn network(host: &str) -> Network {
        Network { host: host.into(), ..Network::default() }
    }

    fn serve(json: &str) -> Value {
        serde_json::from_str(json).unwrap()
    }

    fn entry(port: u16) -> Entry {
        Entry { host_port: format!("box.tail1.ts.net:{port}"), port, proxy: "http://127.0.0.1:3000".into() }
    }

    fn ours(proxy: &str) -> bool {
        is_ours(proxy, &network("0.0.0.0"), &[])
    }

    #[test]
    fn serve_proxying_to_us_is_https() {
        let s = serve(
            r#"{"TCP":{"443":{"HTTPS":true}},"Web":{"box.tail1.ts.net:443":{"Handlers":{"/":{"Proxy":"http://127.0.0.1:3000"}}}}}"#,
        );
        assert_eq!(served_at(&s, &network("0.0.0.0"), &[]), Some(("https://box.tail1.ts.net".into(), false)));
        assert_eq!(served_at(&s, &Network { port: 4000, ..network("0.0.0.0") }, &[]), None);

        let s = serve(
            r#"{"Web":{"box.tail1.ts.net:8443":{"Handlers":{"/":{"Proxy":"3000"}}}},"AllowFunnel":{"box.tail1.ts.net:8443":true}}"#,
        );
        assert_eq!(served_at(&s, &network("127.0.0.1"), &[]), Some(("https://box.tail1.ts.net:8443".into(), true)));

        let s = serve(r#"{"Web":{"box.tail1.ts.net:443":{"Handlers":{"/other":{"Proxy":"http://127.0.0.1:3000"}}}}}"#);
        assert_eq!(served_at(&s, &network("0.0.0.0"), &[]), None);

        let s = serve(
            r#"{"Foreground":{"abc":{"Web":{"box.tail1.ts.net:443":{"Handlers":{"/":{"Proxy":"http://localhost:3000"}}}}}}}"#,
        );
        assert_eq!(served_at(&s, &network("0.0.0.0"), &[]).map(|s| s.0).as_deref(), Some("https://box.tail1.ts.net"));
        assert_eq!(served_at(&Value::Null, &network("0.0.0.0"), &[]), None);
    }

    #[test]
    fn set_and_unset_touch_only_our_entry() {
        let mut s = serve(r#"{"Web":{"box.tail1.ts.net:8443":{"Handlers":{"/":{"Proxy":"http://127.0.0.1:9000"}}}}}"#);
        let theirs = s.clone();

        set(&mut s, &entry(443), true, ours).unwrap();
        assert_eq!(s.pointer("/TCP/443/HTTPS"), Some(&json!(true)));
        assert_eq!(s.pointer("/Web/box.tail1.ts.net:443/Handlers/~1/Proxy"), Some(&json!("http://127.0.0.1:3000")));
        assert_eq!(s.pointer("/AllowFunnel/box.tail1.ts.net:443"), Some(&json!(true)));
        assert!(s.pointer("/Web/box.tail1.ts.net:8443").is_some());

        set(&mut s, &entry(443), false, ours).unwrap();
        assert_eq!(s.get("AllowFunnel"), None);

        unset(&mut s, &entry(443));
        assert_eq!(s, theirs);

        let mut s = Value::Null;
        set(&mut s, &entry(443), false, ours).unwrap();
        unset(&mut s, &entry(443));
        assert_eq!(s, json!({}));
    }

    #[test]
    fn set_adopts_ours_and_refuses_theirs() {
        let mine = r#"{"TCP":{"443":{"HTTPS":true}},"Web":{"box.tail1.ts.net:443":{"Handlers":{"/":{"Proxy":"http://localhost:3000"}}}}}"#;
        let mut s = serve(mine);
        set(&mut s, &entry(443), false, ours).unwrap();
        assert_eq!(s.pointer("/Web/box.tail1.ts.net:443/Handlers/~1/Proxy"), Some(&json!("http://127.0.0.1:3000")));

        let theirs = r#"{"TCP":{"443":{"HTTPS":true}},"Web":{"box.tail1.ts.net:443":{"Handlers":{"/":{"Proxy":"http://127.0.0.1:8080"}}}}}"#;
        let mut s = serve(theirs);
        assert!(set(&mut s, &entry(443), false, ours).is_err());
        assert_eq!(s, serve(theirs));

        let mut s = serve(r#"{"TCP":{"443":{"TCPForward":"127.0.0.1:22"}}}"#);
        assert!(set(&mut s, &entry(443), false, ours).is_err());

        let mut s = serve(r#"{"Web":{"box.tail1.ts.net:443":{"Handlers":{"/":{"Proxy":"http://127.0.0.1:8080"}}}}}"#);
        let before = s.clone();
        unset(&mut s, &entry(443));
        assert_eq!(s, before);
    }

    #[test]
    fn proxy_targets() {
        assert_eq!(proxy_target(&network("0.0.0.0")), "http://127.0.0.1:3000");
        assert_eq!(proxy_target(&network("::")), "http://127.0.0.1:3000");
        assert_eq!(proxy_target(&network("127.0.0.1")), "http://127.0.0.1:3000");
        assert_eq!(proxy_target(&network("::1")), "http://[::1]:3000");
        assert!(is_ours(&proxy_target(&network("::1")), &network("::1"), &[]));
    }

    #[test]
    fn only_reachable_binds_get_a_url() {
        let ips: Vec<IpAddr> = vec!["100.80.100.38".parse().unwrap()];
        assert!(reachable(&network("0.0.0.0"), &ips).is_ok());
        assert!(reachable(&network("::"), &ips).is_ok());
        assert!(reachable(&network("100.80.100.38"), &ips).is_ok());
        assert!(reachable(&network("127.0.0.1"), &ips).is_err());
        assert!(reachable(&network("localhost"), &ips).is_err());
        assert!(reachable(&network("192.168.1.5"), &ips).is_err());
    }

    #[test]
    fn status_parses() {
        let s: LocalStatus = serde_json::from_str(
            r#"{"BackendState":"Running","Self":{"DNSName":"box.tail1.ts.net.","TailscaleIPs":["100.80.100.38","fd7a:115c:a1e0::1"],
                "CapMap":{"https":null,"funnel":null,"https://tailscale.com/cap/funnel-ports?ports=443,8443":null}},
                "CurrentTailnet":{"MagicDNSEnabled":true}}"#,
        )
        .unwrap();
        let caps = s.caps();
        assert!(caps.https && caps.funnel);
        assert_eq!(caps.funnel_ports, vec![443, 8443]);
        assert_eq!(s.me.unwrap().ips.len(), 2);
        assert!(s.current_tailnet.unwrap().magic_dns);

        let s: LocalStatus =
            serde_json::from_str(r#"{"BackendState":"Running","Self":{"Capabilities":["https"]}}"#).unwrap();
        let caps = s.caps();
        assert!(caps.https && !caps.funnel);
        assert_eq!(caps.funnel_ports, FUNNEL_PORTS);
    }
}

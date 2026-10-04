//! Bounded, operator-triggered network diagnostics with fail-closed destination checks.

use std::{
    net::SocketAddr,
    time::{Duration, Instant},
};

use chrono::{DateTime, Utc};
use reqwest::{Client, Url, redirect::Policy};
use serde::{Deserialize, Serialize};
use tokio::{net::TcpStream, time::timeout};
use x509_parser::parse_x509_certificate;

use crate::minecraft_probe::{public_address, valid_host};

const MAX_DNS_ANSWERS: usize = 32;
const MAX_CONNECT_ADDRESSES: usize = 4;
const DEFAULT_TIMEOUT_MS: u16 = 5_000;

#[derive(Debug, Clone, Copy, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum Kind {
    Http,
    Https,
    Tcp,
    Dns,
    Tls,
}

impl Kind {
    fn as_str(self) -> &'static str {
        match self {
            Self::Http => "http",
            Self::Https => "https",
            Self::Tcp => "tcp",
            Self::Dns => "dns",
            Self::Tls => "tls",
        }
    }

    fn default_port(self) -> Option<u16> {
        match self {
            Self::Http => Some(80),
            Self::Https | Self::Tls => Some(443),
            Self::Tcp | Self::Dns => None,
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Request {
    pub(crate) kind: Kind,
    pub(crate) host: String,
    pub(crate) port: Option<u16>,
    pub(crate) path: Option<String>,
    pub(crate) expected_status: Option<u16>,
    pub(crate) timeout_ms: Option<u16>,
}

#[derive(Debug)]
pub(crate) enum Failure {
    InvalidRequest,
    UnsafeDestination,
    CouldNotResolve,
    NoResponse,
    TimedOut,
    InvalidCertificate,
}

impl Failure {
    pub(crate) fn state(&self) -> &'static str {
        match self {
            Self::InvalidRequest => "invalid_request",
            Self::UnsafeDestination => "unsafe_destination",
            Self::CouldNotResolve => "could_not_resolve",
            Self::NoResponse => "no_response",
            Self::TimedOut => "timed_out",
            Self::InvalidCertificate => "invalid_certificate",
        }
    }

    fn message(&self) -> &'static str {
        match self {
            Self::InvalidRequest => "The probe configuration is invalid.",
            Self::UnsafeDestination => {
                "The destination resolves to a private, local, reserved, or mixed-trust address."
            }
            Self::CouldNotResolve => "The destination name could not be resolved.",
            Self::NoResponse => "The destination did not answer before the bounded check ended.",
            Self::TimedOut => "The network check reached its configured timeout.",
            Self::InvalidCertificate => {
                "The TLS certificate was rejected, unavailable, or could not be read safely."
            }
        }
    }
}

#[derive(Debug, Serialize)]
pub(crate) struct ResultData {
    pub(crate) kind: &'static str,
    pub(crate) state: &'static str,
    pub(crate) message: String,
    pub(crate) probe_location: &'static str,
    pub(crate) observed_at: DateTime<Utc>,
    pub(crate) response_ms: Option<u64>,
    pub(crate) status_code: Option<u16>,
    pub(crate) resolved_addresses: u8,
    pub(crate) tls_expires_at: Option<DateTime<Utc>>,
    pub(crate) tls_days_remaining: Option<i64>,
}

impl ResultData {
    pub(crate) fn failed(kind: Kind, failure: &Failure) -> Self {
        Self {
            kind: kind.as_str(),
            state: failure.state(),
            message: failure.message().to_owned(),
            probe_location: "community_control_plane",
            observed_at: Utc::now(),
            response_ms: None,
            status_code: None,
            resolved_addresses: 0,
            tls_expires_at: None,
            tls_days_remaining: None,
        }
    }
}

pub(crate) async fn probe(request: &Request) -> Result<ResultData, Failure> {
    validate(request)?;
    let timeout_duration =
        Duration::from_millis(u64::from(request.timeout_ms.unwrap_or(DEFAULT_TIMEOUT_MS)));
    match timeout(timeout_duration, probe_bounded(request, timeout_duration)).await {
        Ok(result) => result,
        Err(_) => Err(Failure::TimedOut),
    }
}

fn validate(request: &Request) -> Result<(), Failure> {
    if !valid_host(&request.host) || request.port == Some(0) {
        return Err(Failure::InvalidRequest);
    }
    let timeout_ms = request.timeout_ms.unwrap_or(DEFAULT_TIMEOUT_MS);
    if !(250..=10_000).contains(&timeout_ms) {
        return Err(Failure::InvalidRequest);
    }
    if matches!(request.kind, Kind::Tcp) && request.port.is_none() {
        return Err(Failure::InvalidRequest);
    }
    if matches!(request.kind, Kind::Dns)
        && (request.port.is_some() || request.path.is_some() || request.expected_status.is_some())
    {
        return Err(Failure::InvalidRequest);
    }
    if let Some(path) = &request.path
        && (!matches!(request.kind, Kind::Http | Kind::Https | Kind::Tls)
            || path.is_empty()
            || path.len() > 2_048
            || !path.starts_with('/')
            || path.starts_with("//")
            || path.contains(['\r', '\n', '#']))
    {
        return Err(Failure::InvalidRequest);
    }
    if request
        .expected_status
        .is_some_and(|status| !(100..=599).contains(&status))
        || (request.expected_status.is_some() && !matches!(request.kind, Kind::Http | Kind::Https))
    {
        return Err(Failure::InvalidRequest);
    }
    Ok(())
}

async fn probe_bounded(
    request: &Request,
    timeout_duration: Duration,
) -> Result<ResultData, Failure> {
    let resolve_port = request
        .port
        .or_else(|| request.kind.default_port())
        .unwrap_or(1);
    let addresses: Vec<SocketAddr> = tokio::net::lookup_host((&*request.host, resolve_port))
        .await
        .map_err(|_| Failure::CouldNotResolve)?
        .take(MAX_DNS_ANSWERS + 1)
        .collect();
    if addresses.is_empty() || addresses.len() > MAX_DNS_ANSWERS {
        return Err(Failure::CouldNotResolve);
    }
    // Reject the complete answer set before using any address. Connections use only these
    // numeric answers, so DNS cannot redirect a validated request during the same probe.
    validate_addresses(&addresses)?;
    let answer_count = u8::try_from(addresses.len()).unwrap_or(u8::MAX);
    let started = Instant::now();
    match request.kind {
        Kind::Dns => Ok(success(
            request.kind,
            started,
            answer_count,
            "Public DNS answers resolved and passed the destination policy.",
        )),
        Kind::Tcp => probe_tcp(request, &addresses, started, answer_count).await,
        Kind::Http | Kind::Https | Kind::Tls => {
            probe_http(request, &addresses, timeout_duration, started, answer_count).await
        }
    }
}

fn validate_addresses(addresses: &[SocketAddr]) -> Result<(), Failure> {
    if addresses.is_empty()
        || addresses
            .iter()
            .any(|address| !public_address(address.ip()))
    {
        return Err(Failure::UnsafeDestination);
    }
    Ok(())
}

async fn probe_tcp(
    request: &Request,
    addresses: &[SocketAddr],
    started: Instant,
    answer_count: u8,
) -> Result<ResultData, Failure> {
    for address in addresses.iter().take(MAX_CONNECT_ADDRESSES) {
        if TcpStream::connect(address).await.is_ok() {
            return Ok(success(
                request.kind,
                started,
                answer_count,
                "A TCP connection was established to the validated destination.",
            ));
        }
    }
    Err(Failure::NoResponse)
}

async fn probe_http(
    request: &Request,
    addresses: &[SocketAddr],
    timeout_duration: Duration,
    started: Instant,
    answer_count: u8,
) -> Result<ResultData, Failure> {
    let scheme = if matches!(request.kind, Kind::Http) {
        "http"
    } else {
        "https"
    };
    let port = request
        .port
        .or_else(|| request.kind.default_port())
        .ok_or(Failure::InvalidRequest)?;
    let authority = if request.host.contains(':') {
        format!("[{}]:{port}", request.host)
    } else {
        format!("{}:{port}", request.host)
    };
    let url = Url::parse(&format!(
        "{scheme}://{authority}{}",
        request.path.as_deref().unwrap_or("/")
    ))
    .map_err(|_| Failure::InvalidRequest)?;
    if url.host_str() != Some(request.host.as_str())
        || !url.username().is_empty()
        || url.password().is_some()
        || url.fragment().is_some()
    {
        return Err(Failure::InvalidRequest);
    }
    let pinned: Vec<SocketAddr> = addresses
        .iter()
        .copied()
        .take(MAX_CONNECT_ADDRESSES)
        .collect();
    let client = Client::builder()
        .redirect(Policy::none())
        .timeout(timeout_duration)
        .connect_timeout(timeout_duration)
        .tls_info(true)
        .resolve_to_addrs(&request.host, &pinned)
        .build()
        .map_err(|_| Failure::NoResponse)?;
    let response = client
        .get(url)
        .header(reqwest::header::ACCEPT, "*/*")
        .send()
        .await
        .map_err(|error| {
            if error.is_timeout() {
                Failure::TimedOut
            } else if matches!(request.kind, Kind::Https | Kind::Tls) {
                Failure::InvalidCertificate
            } else {
                Failure::NoResponse
            }
        })?;
    let status = response.status().as_u16();
    let expected = request.expected_status;
    let status_ok = expected.map_or_else(|| (200..400).contains(&status), |value| value == status);
    let mut result = success(
        request.kind,
        started,
        answer_count,
        if status_ok {
            "The endpoint responded with the expected HTTP status."
        } else {
            "The endpoint responded, but its HTTP status did not match the check."
        },
    );
    result.status_code = Some(status);
    if !status_ok && !matches!(request.kind, Kind::Tls) {
        result.state = "unexpected_status";
    }
    if matches!(request.kind, Kind::Https | Kind::Tls) {
        let tls = response
            .extensions()
            .get::<reqwest::tls::TlsInfo>()
            .and_then(reqwest::tls::TlsInfo::peer_certificate)
            .ok_or(Failure::InvalidCertificate)?;
        let (_, certificate) =
            parse_x509_certificate(tls).map_err(|_| Failure::InvalidCertificate)?;
        let expires =
            DateTime::<Utc>::from_timestamp(certificate.validity().not_after.timestamp(), 0)
                .ok_or(Failure::InvalidCertificate)?;
        result.tls_expires_at = Some(expires);
        result.tls_days_remaining = Some((expires - Utc::now()).num_days());
        if expires <= Utc::now() {
            result.state = "invalid_certificate";
            "The endpoint responded with an expired TLS certificate."
                .clone_into(&mut result.message);
        } else if matches!(request.kind, Kind::Tls) {
            "The TLS handshake and certificate validation succeeded."
                .clone_into(&mut result.message);
        }
    }
    Ok(result)
}

fn success(kind: Kind, started: Instant, answer_count: u8, message: &'static str) -> ResultData {
    ResultData {
        kind: kind.as_str(),
        state: "responding",
        message: message.to_owned(),
        probe_location: "community_control_plane",
        observed_at: Utc::now(),
        response_ms: Some(u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX)),
        status_code: None,
        resolved_addresses: answer_count,
        tls_expires_at: None,
        tls_days_remaining: None,
    }
}

#[cfg(test)]
mod tests {
    use std::{net::SocketAddr, time::Duration};

    use tokio::{io::AsyncWriteExt, net::TcpListener};

    use super::{Failure, Kind, Request, probe_http, validate, validate_addresses};

    fn request(kind: Kind) -> Request {
        Request {
            kind,
            host: "example.com".to_owned(),
            port: None,
            path: None,
            expected_status: None,
            timeout_ms: None,
        }
    }

    #[test]
    fn validation_rejects_ambiguous_or_unbounded_inputs() {
        let mut check = request(Kind::Http);
        check.path = Some("//metadata.invalid/latest".to_owned());
        assert!(matches!(validate(&check), Err(Failure::InvalidRequest)));
        check.path = Some("/ok\r\nHost: metadata.invalid".to_owned());
        assert!(matches!(validate(&check), Err(Failure::InvalidRequest)));
        check.path = Some(format!("/{}", "a".repeat(2_049)));
        assert!(matches!(validate(&check), Err(Failure::InvalidRequest)));

        let mut dns = request(Kind::Dns);
        dns.port = Some(53);
        assert!(matches!(validate(&dns), Err(Failure::InvalidRequest)));

        let mut tcp = request(Kind::Tcp);
        assert!(matches!(validate(&tcp), Err(Failure::InvalidRequest)));
        tcp.port = Some(22);
        tcp.timeout_ms = Some(249);
        assert!(matches!(validate(&tcp), Err(Failure::InvalidRequest)));
    }

    #[tokio::test]
    async fn private_and_reserved_destinations_fail_closed() {
        for host in ["127.0.0.1", "10.0.0.1", "169.254.169.254", "::1"] {
            let mut check = request(Kind::Tcp);
            check.host = host.to_owned();
            check.port = Some(80);
            assert!(matches!(
                super::probe(&check).await,
                Err(Failure::UnsafeDestination)
            ));
        }
    }

    #[test]
    fn mixed_public_and_private_dns_answers_fail_closed() {
        let answers = [
            SocketAddr::from(([8, 8, 8, 8], 443)),
            SocketAddr::from(([127, 0, 0, 1], 443)),
        ];
        assert!(matches!(
            validate_addresses(&answers),
            Err(Failure::UnsafeDestination)
        ));
    }

    #[tokio::test]
    async fn http_probe_uses_pinned_address_and_does_not_follow_redirects() {
        let bound = TcpListener::bind("127.0.0.1:0").await;
        assert!(bound.is_ok());
        let Ok(listener) = bound else {
            return;
        };
        let address = listener.local_addr();
        assert!(address.is_ok());
        let Ok(address) = address else {
            return;
        };
        let fixture = tokio::spawn(async move {
            let Ok((mut stream, _)) = listener.accept().await else {
                return false;
            };
            stream
                .write_all(
                    b"HTTP/1.1 302 Found\r\nLocation: http://169.254.169.254/latest\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
                )
                .await
                .is_ok()
        });
        let check = Request {
            kind: Kind::Http,
            host: "probe.example".to_owned(),
            port: Some(address.port()),
            path: Some("/health".to_owned()),
            expected_status: Some(200),
            timeout_ms: Some(1_000),
        };
        let result = probe_http(
            &check,
            &[address],
            Duration::from_secs(1),
            std::time::Instant::now(),
            1,
        )
        .await;
        assert!(matches!(
            result,
            Ok(ref data) if data.state == "unexpected_status" && data.status_code == Some(302)
        ));
        assert!(matches!(fixture.await, Ok(true)));
    }
}

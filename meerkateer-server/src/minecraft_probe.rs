//! Bounded Minecraft Java status query for an operator-triggered Community test.
use std::{
    net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr},
    time::{Duration, Instant},
};

use serde::Serialize;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpStream,
    time::timeout,
};

const MAX_PACKET: usize = 65_536;
const MAX_ADDRESSES: usize = 4;

#[derive(Debug)]
pub(crate) enum Failure {
    UnsafeDestination,
    CouldNotResolve,
    NoResponse,
    TimedOut,
    InvalidResponse,
}

impl Failure {
    pub(crate) fn state(&self) -> &'static str {
        match self {
            Self::UnsafeDestination => "unsafe_destination",
            Self::CouldNotResolve => "could_not_resolve",
            Self::NoResponse => "no_response",
            Self::TimedOut => "timed_out",
            Self::InvalidResponse => "unreadable_response",
        }
    }

    pub(crate) fn message(&self) -> &'static str {
        match self {
            Self::UnsafeDestination => "This address is not allowed for a public probe.",
            Self::CouldNotResolve => "The game address could not be resolved from this probe.",
            Self::NoResponse => "The game did not answer this probe; its process state is unknown.",
            Self::TimedOut => {
                "The status query timed out from this probe; its process state is unknown."
            }
            Self::InvalidResponse => {
                "The server answered, but this probe could not read a Minecraft Java status response."
            }
        }
    }
}

#[derive(Debug, Serialize)]
pub(crate) struct ResultData {
    pub(crate) state: &'static str,
    pub(crate) message: &'static str,
    pub(crate) probe_location: &'static str,
    pub(crate) observed_at: chrono::DateTime<chrono::Utc>,
    pub(crate) response_ms: Option<u64>,
    pub(crate) version_name: Option<String>,
    pub(crate) players_online: Option<u32>,
    pub(crate) players_max: Option<u32>,
}

impl ResultData {
    pub(crate) fn failed(failure: &Failure) -> Self {
        Self {
            state: failure.state(),
            message: failure.message(),
            probe_location: "community_control_plane",
            observed_at: chrono::Utc::now(),
            response_ms: None,
            version_name: None,
            players_online: None,
            players_max: None,
        }
    }
}

pub(crate) fn valid_host(host: &str) -> bool {
    if host.len() > 253 || host.is_empty() || host.trim() != host {
        return false;
    }
    if host.parse::<IpAddr>().is_ok() {
        return true;
    }
    if host.eq_ignore_ascii_case("localhost") || host.to_ascii_lowercase().ends_with(".localhost") {
        return false;
    }
    host.split('.').all(|label| {
        (1..=63).contains(&label.len())
            && label
                .bytes()
                .next()
                .is_some_and(|byte| byte.is_ascii_alphanumeric())
            && label
                .bytes()
                .last()
                .is_some_and(|byte| byte.is_ascii_alphanumeric())
            && label
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
    })
}

fn public_address(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => public_v4(v4),
        IpAddr::V6(v6) => public_v6(v6),
    }
}

fn public_v4(ip: Ipv4Addr) -> bool {
    let [a, b, c, _] = ip.octets();
    (1..=223).contains(&a)
        && a != 10
        && a != 127
        && !(a == 100 && (64..=127).contains(&b))
        && !(a == 169 && b == 254)
        && !(a == 172 && (16..=31).contains(&b))
        && !(a == 192 && (b == 0 || b == 168 || (b == 88 && c == 99)))
        && !(a == 198 && (b == 18 || b == 19 || (b == 51 && c == 100)))
        && !(a == 203 && b == 0 && c == 113)
}

fn public_v6(ip: Ipv6Addr) -> bool {
    let segments = ip.segments();
    (0x2000..=0x3fff).contains(&segments[0])
        && !(segments[0] == 0x2001 && segments[1] <= 0x01ff)
        && !(segments[0] == 0x2001 && segments[1] == 0x0db8)
        && segments[0] != 0x2002
}

pub(crate) async fn probe(host: &str, port: u16) -> Result<ResultData, Failure> {
    if !valid_host(host) || port == 0 {
        return Err(Failure::UnsafeDestination);
    }
    match timeout(Duration::from_secs(6), probe_bounded(host, port)).await {
        Ok(result) => result,
        Err(_) => Err(Failure::TimedOut),
    }
}

async fn probe_bounded(host: &str, port: u16) -> Result<ResultData, Failure> {
    let addresses: Vec<SocketAddr> = tokio::net::lookup_host((host, port))
        .await
        .map_err(|_| Failure::CouldNotResolve)?
        .take(33)
        .collect();
    if addresses.is_empty() || addresses.len() > 32 {
        return Err(Failure::CouldNotResolve);
    }
    // Validate every A and AAAA answer before connecting to any of them. Connect to
    // the numeric address so a second DNS resolution cannot redirect the request.
    if addresses
        .iter()
        .any(|address| !public_address(address.ip()))
    {
        return Err(Failure::UnsafeDestination);
    }
    let started = Instant::now();
    for address in addresses.into_iter().take(MAX_ADDRESSES) {
        let Ok(stream) = TcpStream::connect(address).await else {
            continue;
        };
        let status = query_stream(stream, host, port).await?;
        return Ok(ResultData {
            state: "responding",
            message: "Minecraft Java status answered from this probe. Login and gameplay were not tested.",
            probe_location: "community_control_plane",
            observed_at: chrono::Utc::now(),
            response_ms: Some(u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX)),
            version_name: status.version_name,
            players_online: status.players_online,
            players_max: status.players_max,
        });
    }
    Err(Failure::NoResponse)
}

#[derive(Debug)]
struct StatusData {
    version_name: Option<String>,
    players_online: Option<u32>,
    players_max: Option<u32>,
}

fn append_varint(output: &mut Vec<u8>, value: i32) {
    let mut bits = u32::from_ne_bytes(value.to_ne_bytes());
    loop {
        let byte = (bits & 0x7f) as u8;
        bits >>= 7;
        if bits == 0 {
            output.push(byte);
            return;
        }
        output.push(byte | 0x80);
    }
}

fn read_varint(input: &[u8], cursor: &mut usize) -> Result<usize, Failure> {
    let mut value = 0usize;
    for shift in (0..=28).step_by(7) {
        let Some(byte) = input.get(*cursor).copied() else {
            return Err(Failure::InvalidResponse);
        };
        *cursor += 1;
        value |= usize::from(byte & 0x7f) << shift;
        if byte & 0x80 == 0 {
            return Ok(value);
        }
    }
    Err(Failure::InvalidResponse)
}

async fn read_stream_varint(stream: &mut TcpStream) -> Result<usize, Failure> {
    let mut encoded = Vec::with_capacity(5);
    for _ in 0..5 {
        let byte = stream
            .read_u8()
            .await
            .map_err(|_| Failure::InvalidResponse)?;
        encoded.push(byte);
        if byte & 0x80 == 0 {
            return read_varint(&encoded, &mut 0);
        }
    }
    Err(Failure::InvalidResponse)
}

async fn query_stream(mut stream: TcpStream, host: &str, port: u16) -> Result<StatusData, Failure> {
    let mut handshake = Vec::with_capacity(host.len() + 16);
    append_varint(&mut handshake, 0); // Handshake packet ID.
    append_varint(&mut handshake, -1); // Status-only query, no client protocol negotiation.
    append_varint(
        &mut handshake,
        i32::try_from(host.len()).map_err(|_| Failure::InvalidResponse)?,
    );
    handshake.extend_from_slice(host.as_bytes());
    handshake.extend_from_slice(&port.to_be_bytes());
    append_varint(&mut handshake, 1); // Next state: status.

    let mut request = Vec::with_capacity(handshake.len() + 8);
    append_varint(
        &mut request,
        i32::try_from(handshake.len()).map_err(|_| Failure::InvalidResponse)?,
    );
    request.extend(handshake);
    request.extend_from_slice(&[1, 0]); // Empty status request: length 1, packet ID 0.
    stream
        .write_all(&request)
        .await
        .map_err(|_| Failure::NoResponse)?;

    let length = read_stream_varint(&mut stream).await?;
    if length == 0 || length > MAX_PACKET {
        return Err(Failure::InvalidResponse);
    }
    let mut packet = vec![0; length];
    stream
        .read_exact(&mut packet)
        .await
        .map_err(|_| Failure::InvalidResponse)?;
    parse_status_packet(&packet)
}

fn parse_status_packet(packet: &[u8]) -> Result<StatusData, Failure> {
    let mut cursor = 0;
    if read_varint(packet, &mut cursor)? != 0 {
        return Err(Failure::InvalidResponse);
    }
    let json_length = read_varint(packet, &mut cursor)?;
    if json_length == 0 || json_length > MAX_PACKET || cursor + json_length != packet.len() {
        return Err(Failure::InvalidResponse);
    }
    let value: serde_json::Value =
        serde_json::from_slice(&packet[cursor..]).map_err(|_| Failure::InvalidResponse)?;
    let name = value
        .get("version")
        .and_then(|version| version.get("name"))
        .and_then(serde_json::Value::as_str)
        .map(|text| text.chars().take(80).collect());
    let players = value.get("players");
    let count = |key| {
        players
            .and_then(|players| players.get(key))
            .and_then(serde_json::Value::as_u64)
            .and_then(|count| u32::try_from(count).ok())
            .filter(|count| *count <= 1_000_000)
    };
    Ok(StatusData {
        version_name: name,
        players_online: count("online"),
        players_max: count("max"),
    })
}

#[cfg(test)]
mod tests {
    use super::{Failure, append_varint, parse_status_packet, public_address, valid_host};
    use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};
    use tokio::{
        io::{AsyncReadExt, AsyncWriteExt},
        net::{TcpListener, TcpStream},
    };

    #[test]
    fn public_destination_filter_rejects_internal_and_special_addresses() {
        for text in [
            "127.0.0.1",
            "10.1.2.3",
            "172.16.0.1",
            "192.168.1.1",
            "169.254.169.254",
            "100.64.1.1",
            "192.0.2.1",
            "198.51.100.1",
            "203.0.113.1",
            "::1",
            "fc00::1",
            "fe80::1",
            "2001:db8::1",
            "2002:c0a8:0101::",
        ] {
            let parsed = text.parse::<IpAddr>();
            assert!(
                parsed.is_ok_and(|address| !public_address(address)),
                "{text}"
            );
        }
        assert!(public_address(IpAddr::V4(Ipv4Addr::new(8, 8, 8, 8))));
        assert!(public_address(IpAddr::V6(Ipv6Addr::new(
            0x2606, 0x4700, 0, 0, 0, 0, 0, 0x1111
        ))));
        assert!(!valid_host("http://example.com"));
        assert!(!valid_host("localhost"));
        assert!(valid_host("play.example.com"));
    }

    #[test]
    fn status_packet_is_bounded_and_ignores_player_identity() {
        let json = br#"{"version":{"name":"Paper 1.21"},"players":{"online":3,"max":20,"sample":[{"name":"private-player"}]}}"#;
        let mut packet = vec![0];
        append_varint(&mut packet, i32::try_from(json.len()).unwrap_or_default());
        packet.extend_from_slice(json);
        let parsed = parse_status_packet(&packet);
        assert!(
            matches!(parsed, Ok(ref data) if data.players_online == Some(3) && data.players_max == Some(20) && data.version_name.as_deref() == Some("Paper 1.21"))
        );
        packet.pop();
        assert!(matches!(
            parse_status_packet(&packet),
            Err(Failure::InvalidResponse)
        ));
    }

    #[tokio::test]
    async fn status_query_round_trips_with_a_local_protocol_fixture() {
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
        let server = tokio::spawn(async move {
            let Ok((mut stream, _)) = listener.accept().await else {
                return false;
            };
            let mut request = [0_u8; 512];
            let Ok(size) = stream.read(&mut request).await else {
                return false;
            };
            if size < 7 {
                return false;
            }
            let json = br#"{"version":{"name":"Paper test"},"players":{"online":2,"max":10}}"#;
            let mut packet = vec![0];
            append_varint(&mut packet, i32::try_from(json.len()).unwrap_or_default());
            packet.extend_from_slice(json);
            let mut response = Vec::new();
            append_varint(
                &mut response,
                i32::try_from(packet.len()).unwrap_or_default(),
            );
            response.extend(packet);
            stream.write_all(&response).await.is_ok()
        });
        let connected = TcpStream::connect(address).await;
        assert!(connected.is_ok());
        let result = match connected {
            Ok(stream) => super::query_stream(stream, "play.example.com", address.port()).await,
            Err(_) => return,
        };
        assert!(
            matches!(result, Ok(ref status) if status.players_online == Some(2) && status.version_name.as_deref() == Some("Paper test"))
        );
        assert!(matches!(server.await, Ok(true)));
    }
}

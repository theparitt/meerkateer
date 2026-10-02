use std::{
    env,
    error::Error as _,
    fs,
    io::Write,
    net::IpAddr,
    path::{Path, PathBuf},
    time::Duration,
};

use anyhow::Error;
use reqwest::{StatusCode, Url};
use serde::Serialize;
use sysinfo::Disks;
use tokio::{net::lookup_host, time::timeout};
use uuid::Uuid;

use crate::{http_client, validate_server_url};

const DNS_TIMEOUT: Duration = Duration::from_secs(5);
const DISK_FAIL_BYTES: u64 = 64 * 1024 * 1024;
const DISK_WARN_BYTES: u64 = 256 * 1024 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CheckState {
    Pass,
    Info,
    Warn,
    Fail,
}

#[derive(Debug, Clone, Serialize)]
pub struct DiagnosticCheck {
    pub code: &'static str,
    pub state: CheckState,
    pub message: String,
    pub hint: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ConnectionReport {
    pub status: &'static str,
    pub destination: &'static str,
    pub server_url: String,
    pub config_path: String,
    pub checks: Vec<DiagnosticCheck>,
}

impl ConnectionReport {
    pub fn is_ok(&self) -> bool {
        !self
            .checks
            .iter()
            .any(|check| check.state == CheckState::Fail)
    }

    pub fn summary(&self) -> String {
        if self.is_ok() {
            "Self-hosted API is reachable and ready".to_owned()
        } else {
            self.checks
                .iter()
                .find(|check| check.state == CheckState::Fail)
                .map_or_else(
                    || "Connection test failed".to_owned(),
                    |check| format!("{}: {}", check.code, check.message),
                )
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClassifiedError {
    pub code: &'static str,
    pub message: String,
    pub hint: &'static str,
}

pub async fn test_self_hosted_connection(server: &str, config_path: &Path) -> ConnectionReport {
    let mut checks = vec![DiagnosticCheck {
        code: "destination",
        state: CheckState::Pass,
        message: "Local / self-hosted Community control plane selected".to_owned(),
        hint: Some("Meerkateer Cloud is not open yet and cannot be selected.".to_owned()),
    }];

    checks.push(config_storage_check(config_path));
    checks.push(disk_space_check(config_path));
    let proxy = proxy_check();
    let proxy_active = proxy.state == CheckState::Warn;
    checks.push(proxy);

    let server_url = match validate_server_url(server) {
        Ok(url) => {
            checks.push(DiagnosticCheck {
                code: "server_url",
                state: CheckState::Pass,
                message: "API URL policy is valid".to_owned(),
                hint: None,
            });
            url
        }
        Err(error) => {
            checks.push(DiagnosticCheck {
                code: "server_url",
                state: CheckState::Fail,
                message: error.to_string(),
                hint: Some(
                    "Use the API origin only. HTTPS is required except for localhost/127.0.0.1."
                        .to_owned(),
                ),
            });
            return report(server, config_path, checks);
        }
    };

    let mut dns = dns_check(&server_url).await;
    let dns_failed = dns.state == CheckState::Fail;
    if dns_failed && proxy_active {
        dns.state = CheckState::Warn;
        dns.hint = Some(
            "The configured proxy may resolve this hostname remotely; continuing with the HTTP test."
                .to_owned(),
        );
    }
    checks.push(dns);
    if dns_failed && !proxy_active {
        return report(server_url.as_str(), config_path, checks);
    }

    checks.push(api_ready_check(&server_url).await);

    report(server_url.as_str(), config_path, checks)
}

async fn dns_check(server_url: &Url) -> DiagnosticCheck {
    let host = server_url.host_str().unwrap_or_default();
    if host.parse::<IpAddr>().is_ok() {
        return DiagnosticCheck {
            code: "dns",
            state: CheckState::Info,
            message: "API uses an IP address; DNS lookup is not required".to_owned(),
            hint: None,
        };
    }
    let port = server_url.port_or_known_default().unwrap_or(443);
    match timeout(DNS_TIMEOUT, lookup_host((host, port))).await {
        Ok(Ok(mut addresses)) => {
            if addresses.next().is_some() {
                DiagnosticCheck {
                    code: "dns",
                    state: CheckState::Pass,
                    message: format!("Resolved {host}"),
                    hint: None,
                }
            } else {
                unresolved_dns_check(host)
            }
        }
        Ok(Err(_)) => unresolved_dns_check(host),
        Err(_) => DiagnosticCheck {
            code: "dns_timeout",
            state: CheckState::Fail,
            message: format!("DNS lookup for {host} timed out"),
            hint: Some(
                "A VPN, DNS filter, firewall, or disconnected network may be blocking DNS."
                    .to_owned(),
            ),
        },
    }
}

fn unresolved_dns_check(host: &str) -> DiagnosticCheck {
    DiagnosticCheck {
        code: "dns",
        state: CheckState::Fail,
        message: format!("Could not resolve {host}"),
        hint: Some(
            "Check DNS, internet access, VPN DNS, /etc/hosts, or the API hostname.".to_owned(),
        ),
    }
}

async fn api_ready_check(server_url: &Url) -> DiagnosticCheck {
    let Ok(endpoint) = server_url.join("ready") else {
        return DiagnosticCheck {
            code: "ready_url",
            state: CheckState::Fail,
            message: "Could not construct the API readiness URL".to_owned(),
            hint: Some("Check the API URL.".to_owned()),
        };
    };
    let client = match http_client() {
        Ok(client) => client,
        Err(error) => {
            return DiagnosticCheck {
                code: "http_client",
                state: CheckState::Fail,
                message: "Could not initialize the HTTPS client".to_owned(),
                hint: Some(classify_error(&error).hint.to_owned()),
            };
        }
    };
    match client.get(endpoint).send().await {
        Ok(response) => http_status_check(response.status()),
        Err(error) => {
            let classified = classify_reqwest_error(&error);
            DiagnosticCheck {
                code: classified.code,
                state: CheckState::Fail,
                message: classified.message,
                hint: Some(classified.hint.to_owned()),
            }
        }
    }
}

pub fn classify_error(error: &Error) -> ClassifiedError {
    if let Some(reqwest_error) = error
        .chain()
        .find_map(|source| source.downcast_ref::<reqwest::Error>())
    {
        return classify_reqwest_error(reqwest_error);
    }
    if let Some(io_error) = error
        .chain()
        .find_map(|source| source.downcast_ref::<std::io::Error>())
    {
        if is_storage_full(io_error) {
            return ClassifiedError {
                code: "storage_full",
                message: "Local disk has no space left for durable state".to_owned(),
                hint: "Free disk space, then restart the Controller; its pending batch is retained.",
            };
        }
        if io_error.kind() == std::io::ErrorKind::PermissionDenied {
            return ClassifiedError {
                code: "config_permission",
                message: "Controller cannot read or write its protected local configuration"
                    .to_owned(),
                hint: "Run the packaged setup/monitor with the documented privileges and repair the config-directory ACL.",
            };
        }
    }

    let detail = error
        .chain()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join(" ")
        .to_ascii_lowercase();
    if detail.contains("credential has expired") {
        return ClassifiedError {
            code: "credential_expired",
            message: "Machine credential has expired".to_owned(),
            hint: "Re-enroll this computer with a fresh one-time token.",
        };
    }
    if detail.contains("http 401") || detail.contains("http 403") {
        return ClassifiedError {
            code: "authentication_rejected",
            message: "The API rejected this machine credential".to_owned(),
            hint: "Check revocation/expiry and re-enroll only if the credential is no longer valid.",
        };
    }
    if detail.contains("http 429") {
        return ClassifiedError {
            code: "rate_limited",
            message: "The API is rate limiting this Controller".to_owned(),
            hint: "Keep the Controller running; it retains one durable batch and retries later.",
        };
    }
    if detail.contains("http 5") {
        return ClassifiedError {
            code: "api_unavailable",
            message: "The API or its upstream service is unavailable".to_owned(),
            hint: "Check API/PostgreSQL/worker health; the Controller will retry without advancing its sequence.",
        };
    }
    if detail.contains("configuration") || detail.contains("config") {
        return ClassifiedError {
            code: "config_invalid",
            message: "Local Controller configuration is missing or invalid".to_owned(),
            hint: "Run doctor, verify the config path/environment, and use setup to repair enrollment.",
        };
    }
    ClassifiedError {
        code: "delivery_failed",
        message: "Telemetry delivery failed".to_owned(),
        hint: "Open Diagnostics, run the connection test, and inspect the service log for the local cause.",
    }
}

fn classify_reqwest_error(error: &reqwest::Error) -> ClassifiedError {
    let mut details = vec![error.to_string()];
    let mut source = error.source();
    while let Some(current) = source {
        details.push(current.to_string());
        source = current.source();
    }
    let detail = details.join(" ").to_ascii_lowercase();
    if error.is_timeout() {
        return ClassifiedError {
            code: "network_timeout",
            message: "The API connection timed out".to_owned(),
            hint: "Check internet/VPN routing, proxy, firewall, port forwarding, and whether the API is overloaded.",
        };
    }
    if detail.contains("certificate") || detail.contains("tls") {
        return ClassifiedError {
            code: "tls_failed",
            message: "TLS certificate validation or handshake failed".to_owned(),
            hint: "Check the certificate chain, hostname, system clock, TLS-inspecting proxy, and VPN policy.",
        };
    }
    if detail.contains("dns") || detail.contains("resolve") {
        return ClassifiedError {
            code: "dns_failed",
            message: "The API hostname could not be resolved".to_owned(),
            hint: "Check DNS, internet access, VPN DNS, /etc/hosts, or the API hostname.",
        };
    }
    if detail.contains("proxy") {
        return ClassifiedError {
            code: "proxy_failed",
            message: "The configured network proxy rejected or could not route the request"
                .to_owned(),
            hint: "Check HTTP(S)_PROXY and NO_PROXY without placing credentials in the Controller config.",
        };
    }
    ClassifiedError {
        code: "connect_failed",
        message: "Could not open a connection to the API".to_owned(),
        hint: "Check the host/port, API process, firewall, VPN route, container port publishing, and proxy settings.",
    }
}

fn http_status_check(status: StatusCode) -> DiagnosticCheck {
    if status.is_success() {
        return DiagnosticCheck {
            code: "api_ready",
            state: CheckState::Pass,
            message: format!("Readiness endpoint returned HTTP {}", status.as_u16()),
            hint: Some("No credential or telemetry was sent by this test.".to_owned()),
        };
    }
    let (code, hint) = match status {
        StatusCode::NOT_FOUND => (
            "api_path_wrong",
            "Use the Meerkateer API origin, not the Web UI URL or a URL with a path.",
        ),
        StatusCode::UNAUTHORIZED | StatusCode::FORBIDDEN => (
            "gateway_auth",
            "The public /ready endpoint is blocked by a gateway, access policy, or authentication layer.",
        ),
        StatusCode::PROXY_AUTHENTICATION_REQUIRED => (
            "proxy_auth",
            "The network proxy requires authentication. Configure the host proxy securely.",
        ),
        StatusCode::BAD_GATEWAY | StatusCode::SERVICE_UNAVAILABLE | StatusCode::GATEWAY_TIMEOUT => {
            (
                "api_unavailable",
                "The API or reverse-proxy upstream is not ready; check the API and PostgreSQL containers.",
            )
        }
        _ => (
            "unexpected_http",
            "Inspect the reverse proxy and API logs for this status code.",
        ),
    };
    DiagnosticCheck {
        code,
        state: CheckState::Fail,
        message: format!("Readiness endpoint returned HTTP {}", status.as_u16()),
        hint: Some(hint.to_owned()),
    }
}

fn report(server: &str, config_path: &Path, checks: Vec<DiagnosticCheck>) -> ConnectionReport {
    let status = if checks.iter().any(|check| check.state == CheckState::Fail) {
        "error"
    } else {
        "ok"
    };
    ConnectionReport {
        status,
        destination: "self_hosted",
        server_url: server.to_owned(),
        config_path: config_path.display().to_string(),
        checks,
    }
}

fn proxy_check() -> DiagnosticCheck {
    let proxy_present = [
        "HTTPS_PROXY",
        "HTTP_PROXY",
        "ALL_PROXY",
        "https_proxy",
        "http_proxy",
        "all_proxy",
    ]
    .iter()
    .any(|name| env::var_os(name).is_some());
    if proxy_present {
        DiagnosticCheck {
            code: "proxy_environment",
            state: CheckState::Warn,
            message: "A proxy environment is active (values are not displayed)".to_owned(),
            hint: Some(
                "If the test fails, check HTTP(S)_PROXY/ALL_PROXY and NO_PROXY for this API host."
                    .to_owned(),
            ),
        }
    } else {
        DiagnosticCheck {
            code: "proxy_environment",
            state: CheckState::Info,
            message: "No explicit HTTP proxy environment was detected".to_owned(),
            hint: Some("A system VPN can still change DNS and routes.".to_owned()),
        }
    }
}

fn config_storage_check(config_path: &Path) -> DiagnosticCheck {
    let parent = config_path
        .parent()
        .filter(|path| !path.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    let parent_exists = parent.exists();
    let probe_parent = if parent_exists {
        parent.to_path_buf()
    } else {
        nearest_existing_path(parent)
    };
    let result = write_probe(&probe_parent);
    if let Err(error) = result {
        return DiagnosticCheck {
            code: if is_storage_full(&error) {
                "storage_full"
            } else {
                "config_unwritable"
            },
            state: CheckState::Fail,
            message: format!(
                "Cannot safely write Controller state in {}",
                parent.display()
            ),
            hint: Some(if is_storage_full(&error) {
                "Free disk space before enrollment or telemetry delivery.".to_owned()
            } else {
                "Check directory ownership/ACL and run the packaged setup with the documented privileges."
                    .to_owned()
            }),
        };
    }
    DiagnosticCheck {
        code: "config_storage",
        state: if parent_exists {
            CheckState::Pass
        } else {
            CheckState::Info
        },
        message: if parent_exists {
            "Protected local state directory is writable".to_owned()
        } else {
            format!("{} can be created during setup", parent.display())
        },
        hint: (!parent_exists)
            .then(|| "The packaged installer creates and protects this directory.".to_owned()),
    }
}

fn write_probe(directory: &Path) -> std::io::Result<()> {
    let probe = directory.join(format!(".meerkateer-write-test-{}", Uuid::new_v4()));
    let result = (|| {
        let mut file = fs::OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(&probe)?;
        file.write_all(b"ok")?;
        file.sync_all()?;
        fs::remove_file(&probe)
    })();
    if result.is_err() {
        let _ = fs::remove_file(&probe);
    }
    result
}

fn disk_space_check(config_path: &Path) -> DiagnosticCheck {
    let Some((available, total)) = storage_for_path(config_path) else {
        return DiagnosticCheck {
            code: "disk_space",
            state: CheckState::Warn,
            message: "Could not determine free space for the config filesystem".to_owned(),
            hint: Some(
                "Confirm the config volume has free space before leaving the daemon running."
                    .to_owned(),
            ),
        };
    };
    let state = if available < DISK_FAIL_BYTES {
        CheckState::Fail
    } else if available < DISK_WARN_BYTES {
        CheckState::Warn
    } else {
        CheckState::Pass
    };
    DiagnosticCheck {
        code: "disk_space",
        state,
        message: format!(
            "{} free of {} on the Controller state filesystem",
            format_bytes(available),
            format_bytes(total)
        ),
        hint: (state != CheckState::Pass)
            .then(|| "Free space so atomic config and retry-state writes cannot fail.".to_owned()),
    }
}

fn storage_for_path(path: &Path) -> Option<(u64, u64)> {
    let existing = nearest_existing_path(path);
    let disks = Disks::new_with_refreshed_list();
    disks
        .list()
        .iter()
        .filter(|disk| existing.starts_with(disk.mount_point()))
        .max_by_key(|disk| disk.mount_point().as_os_str().len())
        .map(|disk| (disk.available_space(), disk.total_space()))
}

fn nearest_existing_path(path: &Path) -> PathBuf {
    let mut current = path.to_path_buf();
    while !current.exists() {
        if !current.pop() {
            return PathBuf::from(".");
        }
    }
    if current.is_absolute() {
        current
    } else {
        env::current_dir()
            .map(|working_directory| working_directory.join(&current))
            .unwrap_or(current)
    }
}

fn is_storage_full(error: &std::io::Error) -> bool {
    matches!(error.raw_os_error(), Some(28 | 112))
}

#[allow(clippy::cast_precision_loss)]
fn format_bytes(value: u64) -> String {
    const GIB: u64 = 1024 * 1024 * 1024;
    const MIB: u64 = 1024 * 1024;
    if value >= GIB {
        format!("{:.1} GiB", value as f64 / GIB as f64)
    } else {
        format!("{:.1} MiB", value as f64 / MIB as f64)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    #[test]
    fn classifies_storage_and_configuration_errors() {
        let disk_error = Error::new(std::io::Error::from_raw_os_error(28));
        assert_eq!(classify_error(&disk_error).code, "storage_full");

        let permission = Error::new(std::io::Error::new(
            std::io::ErrorKind::PermissionDenied,
            "denied",
        ));
        assert_eq!(classify_error(&permission).code, "config_permission");

        let expired = anyhow::anyhow!("agent credential has expired; re-enroll");
        assert_eq!(classify_error(&expired).code, "credential_expired");

        assert_eq!(
            http_status_check(StatusCode::NOT_FOUND).code,
            "api_path_wrong"
        );
        assert_eq!(
            http_status_check(StatusCode::SERVICE_UNAVAILABLE).code,
            "api_unavailable"
        );
    }

    #[test]
    fn reports_storage_without_exposing_environment_values() {
        let report = config_storage_check(Path::new("."));
        assert_ne!(report.state, CheckState::Fail);
        assert!(!proxy_check().message.contains('='));
    }

    #[tokio::test]
    async fn tests_loopback_readiness_without_credentials() -> anyhow::Result<()> {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
        let address = listener.local_addr()?;
        let server = tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await?;
            let mut request = [0_u8; 1024];
            let size = stream.read(&mut request).await?;
            let request = String::from_utf8_lossy(&request[..size]);
            assert!(request.starts_with("GET /ready "));
            assert!(!request.to_ascii_lowercase().contains("authorization:"));
            stream
                .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\n\r\nOK")
                .await?;
            anyhow::Ok(())
        });
        let config_path = env::temp_dir()
            .join(format!("meerkateer-diagnostic-{}", Uuid::new_v4()))
            .join("agent.json");
        let report = test_self_hosted_connection(
            &format!("http://127.0.0.1:{}/", address.port()),
            &config_path,
        )
        .await;
        assert!(report.is_ok(), "{}", report.summary());
        assert!(
            report
                .checks
                .iter()
                .any(|check| check.code == "api_ready" && check.state == CheckState::Pass)
        );
        server.await??;
        Ok(())
    }

    #[tokio::test]
    async fn invalid_server_url_returns_actionable_failure() {
        let report =
            test_self_hosted_connection("http://192.168.0.148:6510/path", Path::new("agent.json"))
                .await;
        assert!(!report.is_ok());
        assert!(report.checks.iter().any(|check| {
            check.code == "server_url"
                && check.state == CheckState::Fail
                && check
                    .hint
                    .as_deref()
                    .is_some_and(|hint| hint.contains("HTTPS"))
        }));
    }
}

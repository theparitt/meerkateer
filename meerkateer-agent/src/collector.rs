use std::{collections::HashSet, path::Path, time::Duration};

use anyhow::{bail, ensure};
use chrono::{SecondsFormat, Utc};
use serde::Serialize;
use sysinfo::{Disks, MINIMUM_CPU_UPDATE_INTERVAL, ProcessesToUpdate, System};
use tokio::{process::Command, task::JoinSet, time::sleep, time::timeout};

pub const MAX_WATCHED_PROCESSES: usize = 16;
pub const MAX_WATCHED_SERVICES: usize = 16;
const SERVICE_QUERY_TIMEOUT: Duration = Duration::from_secs(3);

#[derive(Debug, Clone, Serialize)]
pub struct ProcessObservation {
    pub name: String,
    pub running: bool,
    pub instances: u32,
}

#[derive(Debug, Clone, Serialize)]
pub struct ServiceObservation {
    pub name: String,
    pub running: Option<bool>,
    pub state: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct HostSnapshot {
    pub collected_at: String,
    pub platform: &'static str,
    pub architecture: &'static str,
    pub cpu_usage_percent: f64,
    pub memory_total_bytes: u64,
    pub memory_used_bytes: u64,
    pub disk_total_bytes: u64,
    pub disk_used_bytes: u64,
    pub disk_count: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub inode_total: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub inode_used: Option<u64>,
    pub watched_processes: Vec<ProcessObservation>,
    pub watched_services: Vec<ServiceObservation>,
}

pub async fn collect(
    watched_processes: &[String],
    watched_services: &[String],
) -> anyhow::Result<HostSnapshot> {
    validate_watched_processes(watched_processes)?;
    validate_watched_services(watched_services)?;
    if !sysinfo::IS_SUPPORTED_SYSTEM {
        bail!("this operating system is not supported by the host collector");
    }

    // CPU usage is a delta. Take two bounded observations rather than reporting
    // a misleading zero from the first refresh.
    let mut system = System::new_all();
    sleep(MINIMUM_CPU_UPDATE_INTERVAL).await;
    system.refresh_cpu_usage();
    system.refresh_memory();
    system.refresh_processes(ProcessesToUpdate::All, true);

    let disks = Disks::new_with_refreshed_list();
    let mut disk_total_bytes = 0_u64;
    let mut disk_available_bytes = 0_u64;
    let mut disk_count = 0_u32;
    let mut inode_total = 0_u64;
    let mut inode_free = 0_u64;
    let mut inode_filesystems = 0_u32;
    let mut seen_disks = HashSet::new();
    for disk in disks
        .list()
        .iter()
        .filter(|disk| !disk.is_removable() && disk.total_space() > 0)
    {
        // Linux bind mounts and container mounts can expose the same backing
        // filesystem many times. Count each device/capacity pair once.
        if !seen_disks.insert((disk.name().to_os_string(), disk.total_space())) {
            continue;
        }
        disk_total_bytes = disk_total_bytes.saturating_add(disk.total_space());
        disk_available_bytes = disk_available_bytes.saturating_add(disk.available_space());
        disk_count = disk_count.saturating_add(1);
        if let Some((filesystem_inodes, filesystem_free)) = inode_capacity(disk.mount_point()) {
            inode_total = inode_total.saturating_add(filesystem_inodes);
            inode_free = inode_free.saturating_add(filesystem_free.min(filesystem_inodes));
            inode_filesystems = inode_filesystems.saturating_add(1);
        }
    }

    let watched_processes = watched_processes
        .iter()
        .map(|requested| {
            let instances = system
                .processes()
                .values()
                .filter(|process| {
                    process
                        .name()
                        .to_string_lossy()
                        .eq_ignore_ascii_case(requested)
                })
                .count();
            let instances = u32::try_from(instances).unwrap_or(u32::MAX);
            ProcessObservation {
                name: requested.clone(),
                running: instances > 0,
                instances,
            }
        })
        .collect();
    let watched_services = collect_services(watched_services).await;

    let cpu_usage_percent = f64::from(system.global_cpu_usage()).clamp(0.0, 100.0);
    let memory_total_bytes = system.total_memory();
    let memory_used_bytes = system.used_memory().min(memory_total_bytes);
    Ok(HostSnapshot {
        collected_at: Utc::now().to_rfc3339_opts(SecondsFormat::Secs, true),
        platform: std::env::consts::OS,
        architecture: std::env::consts::ARCH,
        cpu_usage_percent,
        memory_total_bytes,
        memory_used_bytes,
        disk_total_bytes,
        disk_used_bytes: disk_total_bytes.saturating_sub(disk_available_bytes),
        disk_count,
        inode_total: (inode_filesystems > 0).then_some(inode_total),
        inode_used: (inode_filesystems > 0).then_some(inode_total.saturating_sub(inode_free)),
        watched_processes,
        watched_services,
    })
}

#[cfg(unix)]
fn inode_capacity(path: &std::path::Path) -> Option<(u64, u64)> {
    let stats = rustix::fs::statvfs(path).ok()?;
    (stats.f_files > 0).then_some((stats.f_files, stats.f_ffree))
}

#[cfg(not(unix))]
const fn inode_capacity(_path: &std::path::Path) -> Option<(u64, u64)> {
    None
}

pub fn validate_watched_processes(values: &[String]) -> anyhow::Result<()> {
    ensure!(
        values.len() <= MAX_WATCHED_PROCESSES,
        "at most {MAX_WATCHED_PROCESSES} --watch-process values are allowed"
    );
    let mut unique_names = HashSet::new();
    for value in values {
        let length = value.chars().count();
        ensure!(
            (1..=64).contains(&length) && value.trim() == value,
            "--watch-process must contain 1-64 characters without surrounding whitespace"
        );
        ensure!(
            !value.chars().any(char::is_control),
            "--watch-process must not contain control characters"
        );
        let normalized = value.to_ascii_lowercase();
        ensure!(
            unique_names.insert(normalized.clone()),
            "--watch-process values must be unique (case-insensitive)"
        );
        ensure!(
            ![
                "authorization",
                "password",
                "secret",
                "token",
                "mks_sk_",
                "mka_agent_",
                "mka_enroll_",
            ]
            .iter()
            .any(|forbidden| normalized.contains(forbidden)),
            "--watch-process contains privacy-sensitive text"
        );
    }
    Ok(())
}

pub fn validate_watched_services(values: &[String]) -> anyhow::Result<()> {
    ensure!(
        values.len() <= MAX_WATCHED_SERVICES,
        "at most {MAX_WATCHED_SERVICES} --watch-service values are allowed"
    );
    let mut unique_names = HashSet::new();
    for value in values {
        let length = value.chars().count();
        ensure!(
            (1..=64).contains(&length) && value.trim() == value,
            "--watch-service must contain 1-64 characters without surrounding whitespace"
        );
        ensure!(
            !value.chars().any(char::is_control),
            "--watch-service must not contain control characters"
        );
        let normalized = value.to_ascii_lowercase();
        ensure!(
            unique_names.insert(normalized.clone()),
            "--watch-service values must be unique (case-insensitive)"
        );
        ensure!(
            ![
                "authorization",
                "password",
                "secret",
                "token",
                "mks_sk_",
                "mka_agent_",
                "mka_enroll_",
            ]
            .iter()
            .any(|forbidden| normalized.contains(forbidden)),
            "--watch-service contains privacy-sensitive text"
        );
        validate_platform_service_name(value)?;
    }
    Ok(())
}

#[cfg(target_os = "linux")]
fn validate_platform_service_name(value: &str) -> anyhow::Result<()> {
    ensure!(
        value.ends_with(".service")
            && value
                .chars()
                .all(|character| character.is_ascii_alphanumeric() || "-_.@:".contains(character)),
        "Linux --watch-service values must be exact systemd .service unit names"
    );
    Ok(())
}

#[cfg(target_os = "windows")]
fn validate_platform_service_name(value: &str) -> anyhow::Result<()> {
    ensure!(
        !value
            .chars()
            .any(|character| "\\/:*?\"<>|".contains(character)),
        "Windows --watch-service contains a character unsupported in service names"
    );
    Ok(())
}

#[cfg(not(any(target_os = "linux", target_os = "windows")))]
fn validate_platform_service_name(_value: &str) -> anyhow::Result<()> {
    bail!("service monitoring is supported only on Linux and Windows")
}

async fn collect_services(values: &[String]) -> Vec<ServiceObservation> {
    let mut tasks = JoinSet::new();
    for (index, name) in values.iter().cloned().enumerate() {
        tasks.spawn(async move { (index, observe_service(name).await) });
    }
    let mut observations = vec![None; values.len()];
    while let Some(result) = tasks.join_next().await {
        if let Ok((index, observation)) = result
            && let Some(slot) = observations.get_mut(index)
        {
            *slot = Some(observation);
        }
    }
    observations
        .into_iter()
        .enumerate()
        .map(|(index, observation)| {
            observation.unwrap_or_else(|| ServiceObservation {
                name: values[index].clone(),
                running: None,
                state: "query_failed".to_owned(),
            })
        })
        .collect()
}

#[cfg(target_os = "linux")]
async fn observe_service(name: String) -> ServiceObservation {
    let program = if Path::new("/usr/bin/systemctl").is_file() {
        "/usr/bin/systemctl"
    } else {
        "/bin/systemctl"
    };
    let mut command = Command::new(program);
    command
        .arg("show")
        .arg("--property=LoadState")
        .arg("--property=ActiveState")
        .arg("--")
        .arg(&name)
        .kill_on_drop(true);
    let state = match timeout(SERVICE_QUERY_TIMEOUT, command.output()).await {
        Ok(Ok(output)) => normalized_linux_service_state(&output.stdout, &output.stderr),
        Ok(Err(_)) => "manager_unavailable",
        Err(_) => "query_timeout",
    };
    ServiceObservation {
        name,
        running: service_running_state(state),
        state: state.to_owned(),
    }
}

#[cfg(target_os = "linux")]
fn normalized_linux_service_state(stdout: &[u8], stderr: &[u8]) -> &'static str {
    let output = String::from_utf8_lossy(stdout);
    let load_state = output
        .lines()
        .find_map(|line| line.trim().strip_prefix("LoadState="));
    if load_state == Some("not-found") {
        return "not_found";
    }
    let active_state = output
        .lines()
        .find_map(|line| line.trim().strip_prefix("ActiveState="));
    match active_state {
        Some("active") => "active",
        Some("reloading") => "reloading",
        Some("activating") => "activating",
        Some("deactivating") => "deactivating",
        Some("inactive") => "inactive",
        Some("failed") => "failed",
        _ if String::from_utf8_lossy(stderr)
            .to_ascii_lowercase()
            .contains("permission denied") =>
        {
            "permission_denied"
        }
        _ => "unknown",
    }
}

#[cfg(target_os = "windows")]
async fn observe_service(name: String) -> ServiceObservation {
    let system_root = std::env::var_os("SystemRoot").unwrap_or_else(|| "C:\\Windows".into());
    let program = Path::new(&system_root).join("System32").join("sc.exe");
    let mut command = Command::new(program);
    command.arg("query").arg(&name).kill_on_drop(true);
    let state = match timeout(SERVICE_QUERY_TIMEOUT, command.output()).await {
        Ok(Ok(output)) => normalized_windows_service_state(&output.stdout, &output.stderr),
        Ok(Err(_)) => "manager_unavailable",
        Err(_) => "query_timeout",
    };
    ServiceObservation {
        name,
        running: service_running_state(state),
        state: state.to_owned(),
    }
}

#[cfg(target_os = "windows")]
fn normalized_windows_service_state(stdout: &[u8], stderr: &[u8]) -> &'static str {
    let output = String::from_utf8_lossy(stdout).to_ascii_uppercase();
    for (needle, state) in [
        (" RUNNING", "active"),
        (" START_PENDING", "activating"),
        (" STOP_PENDING", "deactivating"),
        (" PAUSE_PENDING", "deactivating"),
        (" CONTINUE_PENDING", "activating"),
        (" PAUSED", "inactive"),
        (" STOPPED", "inactive"),
    ] {
        if output.contains(needle) {
            return state;
        }
    }
    let error = String::from_utf8_lossy(stderr).to_ascii_lowercase();
    if error.contains("access is denied") {
        "permission_denied"
    } else if error.contains("does not exist") {
        "not_found"
    } else {
        "unknown"
    }
}

#[cfg(not(any(target_os = "linux", target_os = "windows")))]
async fn observe_service(name: String) -> ServiceObservation {
    ServiceObservation {
        name,
        running: None,
        state: "unsupported_platform".to_owned(),
    }
}

const fn service_running_state(state: &str) -> Option<bool> {
    match state.as_bytes() {
        b"active" | b"reloading" => Some(true),
        b"activating" | b"deactivating" | b"inactive" | b"failed" => Some(false),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn watched_processes_are_bounded_and_privacy_safe() {
        assert!(validate_watched_processes(&["java".to_owned(), "server.exe".to_owned()]).is_ok());
        assert!(validate_watched_processes(&[" access-token-worker ".to_owned()]).is_err());
        assert!(validate_watched_processes(&["java".to_owned(), "JAVA".to_owned()]).is_err());
        assert!(
            validate_watched_processes(
                &(0..=MAX_WATCHED_PROCESSES)
                    .map(|index| format!("process-{index}"))
                    .collect::<Vec<_>>()
            )
            .is_err()
        );
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn systemd_service_names_and_states_are_bounded() {
        assert!(
            validate_watched_services(&[
                "minecraft@arena.service".to_owned(),
                "postgresql.service".to_owned(),
            ])
            .is_ok()
        );
        assert!(validate_watched_services(&["postgresql".to_owned()]).is_err());
        assert!(validate_watched_services(&["../secret.service".to_owned()]).is_err());
        assert_eq!(
            normalized_linux_service_state(b"LoadState=loaded\nActiveState=active\n", b""),
            "active"
        );
        assert_eq!(
            normalized_linux_service_state(b"LoadState=loaded\nActiveState=activating\n", b""),
            "activating"
        );
        assert_eq!(
            normalized_linux_service_state(b"", b"Permission denied"),
            "permission_denied"
        );
        assert_eq!(
            normalized_linux_service_state(b"LoadState=not-found\nActiveState=inactive\n", b""),
            "not_found"
        );
        assert_eq!(service_running_state("active"), Some(true));
        assert_eq!(service_running_state("failed"), Some(false));
        assert_eq!(service_running_state("permission_denied"), None);
    }

    #[tokio::test]
    async fn local_snapshot_is_bounded_and_self_consistent() -> anyhow::Result<()> {
        let snapshot = collect(&[], &[]).await?;
        assert!((0.0..=100.0).contains(&snapshot.cpu_usage_percent));
        assert!(snapshot.memory_used_bytes <= snapshot.memory_total_bytes);
        assert!(snapshot.disk_used_bytes <= snapshot.disk_total_bytes);
        if let (Some(used), Some(total)) = (snapshot.inode_used, snapshot.inode_total) {
            assert!(used <= total);
        }
        assert!(snapshot.watched_processes.is_empty());
        assert!(snapshot.watched_services.is_empty());
        Ok(())
    }
}

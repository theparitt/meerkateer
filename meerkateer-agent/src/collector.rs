use std::collections::HashSet;

use anyhow::{bail, ensure};
use chrono::{SecondsFormat, Utc};
use serde::Serialize;
use sysinfo::{Disks, MINIMUM_CPU_UPDATE_INTERVAL, ProcessesToUpdate, System};
use tokio::time::sleep;

pub const MAX_WATCHED_PROCESSES: usize = 16;

#[derive(Debug, Clone, Serialize)]
pub struct ProcessObservation {
    pub name: String,
    pub running: bool,
    pub instances: u32,
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
}

pub async fn collect(watched_processes: &[String]) -> anyhow::Result<HostSnapshot> {
    validate_watched_processes(watched_processes)?;
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

    #[tokio::test]
    async fn local_snapshot_is_bounded_and_self_consistent() -> anyhow::Result<()> {
        let snapshot = collect(&[]).await?;
        assert!((0.0..=100.0).contains(&snapshot.cpu_usage_percent));
        assert!(snapshot.memory_used_bytes <= snapshot.memory_total_bytes);
        assert!(snapshot.disk_used_bytes <= snapshot.disk_total_bytes);
        if let (Some(used), Some(total)) = (snapshot.inode_used, snapshot.inode_total) {
            assert!(used <= total);
        }
        assert!(snapshot.watched_processes.is_empty());
        Ok(())
    }
}

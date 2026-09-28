//! Pure domain rules shared by the Meerkateer control-plane components.

/// Stable current state shown to operators and used by alert evaluation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ServiceState {
    Online,
    Degraded,
    Offline,
    Maintenance,
    Unknown,
}

/// Derive the service state from dependency results and operator context.
#[must_use]
pub fn derive_service_state(
    check_results: impl IntoIterator<Item = bool>,
    telemetry_is_stale: bool,
    in_maintenance: bool,
) -> ServiceState {
    if in_maintenance {
        return ServiceState::Maintenance;
    }
    if telemetry_is_stale {
        return ServiceState::Unknown;
    }

    let mut saw_check = false;
    for is_ok in check_results {
        saw_check = true;
        if !is_ok {
            return ServiceState::Degraded;
        }
    }

    if saw_check {
        ServiceState::Online
    } else {
        ServiceState::Unknown
    }
}

#[cfg(test)]
mod tests {
    use super::{ServiceState, derive_service_state};

    #[test]
    fn maintenance_overrides_staleness_and_checks() {
        assert_eq!(
            derive_service_state([false], true, true),
            ServiceState::Maintenance
        );
    }

    #[test]
    fn stale_telemetry_is_unknown() {
        assert_eq!(
            derive_service_state([true], true, false),
            ServiceState::Unknown
        );
    }

    #[test]
    fn any_failed_check_is_degraded() {
        assert_eq!(
            derive_service_state([true, false], false, false),
            ServiceState::Degraded
        );
    }

    #[test]
    fn all_checks_online() {
        assert_eq!(
            derive_service_state([true, true], false, false),
            ServiceState::Online
        );
    }
}

//! Shared building blocks for every Stratum service: the telemetry domain model
//! (STR-105), configuration (STR-106) and logging setup (STR-107).

/// Returns the one-line startup banner a service prints, e.g. `stratum-worker v0.1.0`.
pub fn banner(component: &str, version: &str) -> String {
    format!("{component} v{version}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn banner_contains_name_and_version() {
        assert_eq!(banner("stratum-worker", "0.1.0"), "stratum-worker v0.1.0");
    }
}

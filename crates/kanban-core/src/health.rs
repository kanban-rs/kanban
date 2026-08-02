/// The health state of a backend or server component.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HealthStatus {
    Healthy,
    Degraded(String),
    Unhealthy(String),
}

impl HealthStatus {
    pub fn is_healthy(&self) -> bool {
        matches!(self, Self::Healthy)
    }

    pub fn is_degraded(&self) -> bool {
        matches!(self, Self::Degraded(_))
    }

    pub fn is_unhealthy(&self) -> bool {
        matches!(self, Self::Unhealthy(_))
    }
}

impl std::fmt::Display for HealthStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Healthy => write!(f, "healthy"),
            Self::Degraded(reason) => write!(f, "degraded: {reason}"),
            Self::Unhealthy(reason) => write!(f, "unhealthy: {reason}"),
        }
    }
}

/// Implemented by backends that can report their own health.
pub trait HealthChecker: Send + Sync {
    fn check(&self) -> HealthStatus;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_healthy_status_is_healthy() {
        assert!(HealthStatus::Healthy.is_healthy());
        assert!(!HealthStatus::Healthy.is_degraded());
    }

    #[test]
    fn test_degraded_status_is_degraded() {
        let s = HealthStatus::Degraded("disk full".into());
        assert!(s.is_degraded());
        assert!(!s.is_healthy());
    }

    #[test]
    fn test_unhealthy_status_is_unhealthy() {
        let s = HealthStatus::Unhealthy("connection lost".into());
        assert!(s.is_unhealthy());
        assert!(!s.is_healthy());
        assert!(!s.is_degraded());
    }

    #[test]
    fn test_healthy_and_degraded_are_not_unhealthy() {
        assert!(!HealthStatus::Healthy.is_unhealthy());
        assert!(!HealthStatus::Degraded("slow".into()).is_unhealthy());
    }

    #[test]
    fn test_health_status_display_healthy() {
        assert_eq!(HealthStatus::Healthy.to_string(), "healthy");
    }

    #[test]
    fn test_health_status_display_degraded() {
        assert_eq!(
            HealthStatus::Degraded("slow".into()).to_string(),
            "degraded: slow"
        );
    }

    #[test]
    fn test_health_status_display_unhealthy() {
        assert_eq!(
            HealthStatus::Unhealthy("gone".into()).to_string(),
            "unhealthy: gone"
        );
    }

    #[test]
    fn test_health_checker_trait_is_object_safe() {
        struct AlwaysHealthy;
        impl HealthChecker for AlwaysHealthy {
            fn check(&self) -> HealthStatus {
                HealthStatus::Healthy
            }
        }
        let _: &dyn HealthChecker = &AlwaysHealthy;
    }
}

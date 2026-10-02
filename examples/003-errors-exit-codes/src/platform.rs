//! Simulated platform API with its own error type.

use crate::manifest::Manifest;

const KNOWN_SERVICES: [&str; 3] = ["billing", "search", "notifications"];

#[derive(Debug, thiserror::Error)]
pub enum PlatformError {
    #[error("service `{0}` is not registered on the platform")]
    UnknownService(String),

    #[error("environment `{0}` requires approval; you are not allowed to deploy there")]
    PermissionDenied(String),

    /// Transient – retrying later may succeed. Scripts/CI can react to its exit code.
    #[error("platform API did not respond within {0}s")]
    Timeout(u64),
}

pub fn deploy(
    manifest: &Manifest,
    env: &str,
    simulate_timeout: bool,
) -> Result<String, PlatformError> {
    if !KNOWN_SERVICES.contains(&manifest.service.as_str()) {
        return Err(PlatformError::UnknownService(manifest.service.clone()));
    }
    if env == "prod" {
        return Err(PlatformError::PermissionDenied(env.to_string()));
    }
    if simulate_timeout {
        return Err(PlatformError::Timeout(30));
    }
    Ok(format!("deploy-{}-{}", manifest.service, manifest.tag))
}

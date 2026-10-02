//! Data shown by the example.

use std::fmt;

use serde::Serialize;

/// The public, machine-readable shape of a service. Field names here ARE the JSON/YAML API:
/// renaming a field breaks every script that reads it.
#[derive(Debug, Clone, Serialize)]
pub struct Service {
    pub name: String,
    pub env: String,
    pub version: String,
    pub replicas: u32,
    pub status: Status,
    /// RFC 3339 timestamp in UTC – unambiguous for machines; format for humans if needed.
    pub updated_at: String,
}

// Enums serialize as lowercase strings ("running"), not numbers – self-describing output.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Status {
    Running,
    Degraded,
    Stopped,
}

impl fmt::Display for Status {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Running => "running",
            Self::Degraded => "degraded",
            Self::Stopped => "stopped",
        })
    }
}

pub fn sample() -> Vec<Service> {
    let service = |name: &str, env: &str, version: &str, replicas, status, updated: &str| Service {
        name: name.into(),
        env: env.into(),
        version: version.into(),
        replicas,
        status,
        updated_at: updated.into(),
    };
    vec![
        service(
            "billing",
            "prod",
            "1.4.2",
            3,
            Status::Running,
            "2026-09-30T12:00:00Z",
        ),
        service(
            "search",
            "prod",
            "2.0.0",
            5,
            Status::Running,
            "2026-09-29T08:30:00Z",
        ),
        service(
            "notifications",
            "staging",
            "0.9.3",
            1,
            Status::Degraded,
            "2026-10-01T17:45:00Z",
        ),
        service(
            "reports",
            "dev",
            "0.4.0",
            0,
            Status::Stopped,
            "2026-08-14T09:00:00Z",
        ),
    ]
}

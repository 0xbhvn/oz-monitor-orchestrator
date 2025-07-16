//! Tenant-related models

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Tenant information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TenantInfo {
    /// Unique tenant identifier
    pub id: Uuid,

    /// Tenant name
    pub name: String,

    /// Tenant status
    pub status: TenantStatus,

    /// Tenant priority
    pub priority: TenantPriority,

    /// Maximum monitors allowed
    pub max_monitors: usize,

    /// Maximum RPC requests per minute
    pub max_rpc_requests_per_minute: u32,

    /// Creation timestamp
    pub created_at: DateTime<Utc>,

    /// Last activity timestamp
    pub last_active_at: DateTime<Utc>,
}

/// Tenant status
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum TenantStatus {
    /// Tenant is active and can process monitors
    Active,

    /// Tenant is suspended (e.g., for non-payment)
    Suspended,

    /// Tenant is in trial period
    Trial,

    /// Tenant is inactive
    Inactive,
}

/// Tenant priority level
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, PartialOrd)]
#[serde(rename_all = "snake_case")]
pub enum TenantPriority {
    /// Highest priority (e.g., enterprise customers)
    Critical = 4,

    /// High priority
    High = 3,

    /// Normal priority (default)
    Normal = 2,

    /// Low priority (e.g., free tier)
    Low = 1,
}

impl Default for TenantPriority {
    fn default() -> Self {
        TenantPriority::Normal
    }
}

impl TenantInfo {
    /// Check if tenant is active
    pub fn is_active(&self) -> bool {
        matches!(self.status, TenantStatus::Active | TenantStatus::Trial)
    }

    /// Get priority as numeric value
    pub fn priority_value(&self) -> u8 {
        self.priority as u8
    }
}

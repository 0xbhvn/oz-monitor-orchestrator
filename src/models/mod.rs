//! Data models for the orchestrator
//!
//! This module contains all the data structures used throughout the orchestrator,
//! organized similarly to OpenZeppelin Monitor's models structure.

pub mod assignment;
pub mod error;
pub mod metrics;
pub mod tenant;

// Re-export main types
pub use assignment::{AssignmentReason, TenantAssignment, WorkerAssignment};
pub use error::ModelError;
pub use metrics::{SystemMetrics, TenantMetrics, WorkerMetrics};
pub use tenant::{TenantInfo, TenantPriority, TenantStatus};

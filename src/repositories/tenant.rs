//! Tenant-Aware Repository Implementation
//!
//! Implements OpenZeppelin Monitor's repository traits with multi-tenant
//! support, loading configurations from the database instead of files.

use anyhow::{Context, Result};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use sqlx::{FromRow, PgPool};
use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;
use uuid::Uuid;

// Import OpenZeppelin Monitor types
use openzeppelin_monitor::{
    models::{Monitor, Network, Trigger},
    repositories::{
        MonitorRepositoryTrait, NetworkRepositoryTrait, NetworkService,
        RepositoryError as OzRepositoryError, TriggerRepositoryTrait, TriggerService,
    },
};

// Import our own repository error for conversions
use crate::repositories::error::RepositoryError;

/// Convert our RepositoryError to OpenZeppelin Monitor's RepositoryError
fn to_oz_error(err: RepositoryError) -> OzRepositoryError {
    match err {
        RepositoryError::ConnectionError(msg) => OzRepositoryError::internal_error(
            format!("Database connection error: {}", msg),
            None,
            None,
        ),
        RepositoryError::QueryError(msg) => OzRepositoryError::internal_error(
            format!("Query execution failed: {}", msg),
            None,
            None,
        ),
        RepositoryError::NotFound { entity_type, id } => {
            OzRepositoryError::load_error(format!("{} not found: {}", entity_type, id), None, None)
        }
        RepositoryError::TenantNotFound(id) => {
            OzRepositoryError::load_error(format!("Tenant not found: {}", id), None, None)
        }
        RepositoryError::SerializationError(msg) => {
            OzRepositoryError::validation_error(format!("Serialization error: {}", msg), None, None)
        }
        RepositoryError::TransactionError(msg) => {
            OzRepositoryError::internal_error(format!("Transaction failed: {}", msg), None, None)
        }
        RepositoryError::ConstraintViolation(msg) => OzRepositoryError::validation_error(
            format!("Constraint violation: {}", msg),
            None,
            None,
        ),
    }
}

/// Convert anyhow::Error to OpenZeppelin Monitor's RepositoryError
fn anyhow_to_oz_error(err: anyhow::Error) -> OzRepositoryError {
    OzRepositoryError::internal_error(err.to_string(), None, None)
}

/// Execute an async function in a sync context
fn execute_async<F, T>(future: F) -> T
where
    F: std::future::Future<Output = T> + Send,
    T: Send,
{
    // Instead of creating a new runtime, use Handle::try_current() to check if we're
    // already in a runtime context
    if let Ok(handle) = tokio::runtime::Handle::try_current() {
        // We're already in a runtime, use block_in_place
        tokio::task::block_in_place(move || handle.block_on(future))
    } else {
        // We're not in a runtime, create a temporary one
        tokio::runtime::Runtime::new()
            .expect("Failed to create runtime")
            .block_on(future)
    }
}

/// Database model for tenant monitors
#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
struct DbMonitor {
    id: Uuid,
    tenant_id: Uuid,
    monitor_id: String,
    name: String,
    networks: Vec<String>,
    configuration: JsonValue,
    is_active: bool,
    created_at: chrono::DateTime<chrono::Utc>,
    updated_at: chrono::DateTime<chrono::Utc>,
}

/// Database model for networks
#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
struct DbNetwork {
    id: Uuid,
    tenant_id: Uuid,
    network_id: String,
    name: String,
    blockchain: String,
    configuration: JsonValue,
    is_active: bool,
    created_at: chrono::DateTime<chrono::Utc>,
    updated_at: chrono::DateTime<chrono::Utc>,
}

/// Database model for triggers
#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
struct DbTrigger {
    id: Uuid,
    tenant_id: Uuid,
    trigger_id: String,
    monitor_id: Uuid,
    name: String,
    #[serde(rename = "type")]
    trigger_type: String,
    configuration: JsonValue,
    is_active: bool,
    created_at: chrono::DateTime<chrono::Utc>,
    updated_at: chrono::DateTime<chrono::Utc>,
}

/// Tenant-aware monitor repository
#[derive(Clone)]
pub struct TenantAwareMonitorRepository {
    db: Arc<PgPool>,
    tenant_filter: Vec<Uuid>,
}

impl TenantAwareMonitorRepository {
    pub fn new(db: Arc<PgPool>, tenant_filter: Vec<Uuid>) -> Self {
        Self { db, tenant_filter }
    }

    /// Update the tenant filter for this repository
    pub async fn update_tenant_filter(&self, _tenant_filter: Vec<Uuid>) {
        // Since tenant_filter is not mutable, we would need to use Arc<RwLock>
        // For now, this is a no-op as the repository is created with specific tenants
        // In a real implementation, you'd want to make tenant_filter mutable
    }

    /// Convert database monitor to OpenZeppelin monitor format
    fn db_to_oz_monitor(&self, db_monitor: DbMonitor) -> Result<Monitor> {
        let config: Monitor = serde_json::from_value(db_monitor.configuration)
            .context("Failed to deserialize monitor configuration")?;

        Ok(Monitor {
            name: db_monitor.name,
            networks: db_monitor.networks,
            ..config
        })
    }
}

#[async_trait]
impl MonitorRepositoryTrait<TenantAwareNetworkRepository, TenantAwareTriggerRepository>
    for TenantAwareMonitorRepository
{
    async fn new(
        _path: Option<&Path>,
        _network_service: Option<NetworkService<TenantAwareNetworkRepository>>,
        _trigger_service: Option<TriggerService<TenantAwareTriggerRepository>>,
    ) -> Result<Self, OzRepositoryError>
    where
        Self: Sized,
    {
        // This doesn't make sense for database-backed storage
        // The repository should be created with new() and tenant_filter
        Err(OzRepositoryError::load_error(
            "Direct construction not supported - use TenantAwareMonitorRepository::new()",
            None,
            None,
        ))
    }

    async fn load_all(
        _path: Option<&Path>,
        _network_service: Option<NetworkService<TenantAwareNetworkRepository>>,
        _trigger_service: Option<TriggerService<TenantAwareTriggerRepository>>,
    ) -> Result<HashMap<String, Monitor>, OzRepositoryError> {
        // This doesn't make sense for database-backed storage
        Err(OzRepositoryError::load_error(
            "Static loading not supported - use instance methods",
            None,
            None,
        ))
    }

    fn get_all(&self) -> HashMap<String, Monitor> {
        execute_async(async {
            match self.get_all_internal().await {
                Ok(monitors) => monitors,
                Err(e) => {
                    tracing::error!("Failed to get monitors: {}", e);
                    HashMap::new()
                }
            }
        })
    }

    fn get(&self, name: &str) -> Option<Monitor> {
        execute_async(async {
            match self.get_internal(name).await {
                Ok(monitor) => monitor,
                Err(e) => {
                    tracing::error!("Failed to get monitor {}: {}", name, e);
                    None
                }
            }
        })
    }

    async fn load_from_path(
        &self,
        _path: Option<&Path>,
        _network_service: Option<NetworkService<TenantAwareNetworkRepository>>,
        _trigger_service: Option<TriggerService<TenantAwareTriggerRepository>>,
    ) -> Result<Monitor, OzRepositoryError> {
        // This method doesn't make sense for database-backed storage
        Err(OzRepositoryError::load_error(
            "Path-based loading not supported",
            None,
            None,
        ))
    }
}

impl TenantAwareMonitorRepository {
    async fn get_all_internal(&self) -> Result<HashMap<String, Monitor>, OzRepositoryError> {
        let monitors = sqlx::query_as!(
            DbMonitor,
            r#"
            SELECT 
                m.id, m.tenant_id, m.monitor_id, m.name, 
                ARRAY[n.network_id]::TEXT[] as "networks!", 
                m.configuration, 
                m.is_active as "is_active!",
                m.created_at as "created_at!",
                m.updated_at as "updated_at!"
            FROM tenant_monitors m
            JOIN tenant_networks n ON m.network_id = n.id
            WHERE m.tenant_id = ANY($1) AND m.is_active = true
            "#,
            &self.tenant_filter[..]
        )
        .fetch_all(&*self.db)
        .await
        .map_err(|e| to_oz_error(RepositoryError::from(e)))?;

        let mut result = HashMap::new();
        for db_monitor in monitors {
            let name = db_monitor.name.clone();
            let monitor = self
                .db_to_oz_monitor(db_monitor)
                .map_err(anyhow_to_oz_error)?;
            result.insert(name, monitor);
        }

        Ok(result)
    }

    async fn get_internal(&self, name: &str) -> Result<Option<Monitor>, OzRepositoryError> {
        let db_monitor = sqlx::query_as!(
            DbMonitor,
            r#"
            SELECT 
                m.id, m.tenant_id, m.monitor_id, m.name, 
                ARRAY[n.network_id]::TEXT[] as "networks!", 
                m.configuration, 
                m.is_active as "is_active!",
                m.created_at as "created_at!",
                m.updated_at as "updated_at!"
            FROM tenant_monitors m
            JOIN tenant_networks n ON m.network_id = n.id
            WHERE m.tenant_id = ANY($1) 
                AND m.name = $2 
                AND m.is_active = true
            LIMIT 1
            "#,
            &self.tenant_filter[..],
            name
        )
        .fetch_optional(&*self.db)
        .await
        .map_err(|e| to_oz_error(RepositoryError::from(e)))?;

        match db_monitor {
            Some(db_monitor) => Ok(Some(
                self.db_to_oz_monitor(db_monitor)
                    .map_err(anyhow_to_oz_error)?,
            )),
            None => Ok(None),
        }
    }
}

/// Tenant-aware network repository
#[derive(Clone)]
pub struct TenantAwareNetworkRepository {
    db: Arc<PgPool>,
    tenant_filter: Vec<Uuid>,
}

impl TenantAwareNetworkRepository {
    pub fn new(db: Arc<PgPool>, tenant_filter: Vec<Uuid>) -> Self {
        Self { db, tenant_filter }
    }

    /// Update the tenant filter for this repository
    pub async fn update_tenant_filter(&self, _tenant_filter: Vec<Uuid>) {
        // Since tenant_filter is not mutable, we would need to use Arc<RwLock>
        // For now, this is a no-op as the repository is created with specific tenants
        // In a real implementation, you'd want to make tenant_filter mutable
    }

    fn db_to_oz_network(&self, db_network: DbNetwork) -> Result<Network> {
        let network: Network = serde_json::from_value(db_network.configuration)
            .context("Failed to deserialize network configuration")?;

        Ok(network)
    }
}

#[async_trait]
impl NetworkRepositoryTrait for TenantAwareNetworkRepository {
    async fn new(_path: Option<&Path>) -> Result<Self, OzRepositoryError>
    where
        Self: Sized,
    {
        // This doesn't make sense for database-backed storage
        Err(OzRepositoryError::load_error(
            "Direct construction not supported - use TenantAwareNetworkRepository::new()",
            None,
            None,
        ))
    }

    async fn load_all(_path: Option<&Path>) -> Result<HashMap<String, Network>, OzRepositoryError> {
        // This doesn't make sense for database-backed storage
        Err(OzRepositoryError::load_error(
            "Static loading not supported - use instance methods",
            None,
            None,
        ))
    }

    fn get_all(&self) -> HashMap<String, Network> {
        execute_async(async {
            match self.get_all_internal().await {
                Ok(networks) => networks,
                Err(e) => {
                    tracing::error!("Failed to get networks: {}", e);
                    HashMap::new()
                }
            }
        })
    }

    fn get(&self, network_id: &str) -> Option<Network> {
        execute_async(async {
            match self.get_internal(network_id).await {
                Ok(network) => network,
                Err(e) => {
                    tracing::error!("Failed to get network {}: {}", network_id, e);
                    None
                }
            }
        })
    }
}

impl TenantAwareNetworkRepository {
    async fn get_all_internal(&self) -> Result<HashMap<String, Network>, OzRepositoryError> {
        let networks = sqlx::query_as!(
            DbNetwork,
            r#"
            SELECT 
                id, tenant_id, network_id, name, blockchain, 
                configuration, 
                is_active as "is_active!", 
                created_at as "created_at!", 
                updated_at as "updated_at!"
            FROM tenant_networks 
            WHERE tenant_id = ANY($1) AND is_active = true
            "#,
            &self.tenant_filter[..]
        )
        .fetch_all(&*self.db)
        .await
        .map_err(|e| to_oz_error(RepositoryError::from(e)))?;

        let mut result = HashMap::new();
        for db_network in networks {
            let network = self
                .db_to_oz_network(db_network)
                .map_err(anyhow_to_oz_error)?;
            result.insert(network.slug.clone(), network);
        }

        Ok(result)
    }

    async fn get_internal(&self, network_id: &str) -> Result<Option<Network>, OzRepositoryError> {
        let db_network = sqlx::query_as!(
            DbNetwork,
            r#"
            SELECT 
                id, tenant_id, network_id, name, blockchain,
                configuration, 
                is_active as "is_active!", 
                created_at as "created_at!", 
                updated_at as "updated_at!"
            FROM tenant_networks 
            WHERE tenant_id = ANY($1) 
                AND network_id = $2 
                AND is_active = true
            LIMIT 1
            "#,
            &self.tenant_filter[..],
            network_id
        )
        .fetch_optional(&*self.db)
        .await
        .map_err(|e| to_oz_error(RepositoryError::from(e)))?;

        match db_network {
            Some(db_network) => Ok(Some(
                self.db_to_oz_network(db_network)
                    .map_err(anyhow_to_oz_error)?,
            )),
            None => Ok(None),
        }
    }
}

/// Tenant-aware trigger repository
#[derive(Clone)]
pub struct TenantAwareTriggerRepository {
    db: Arc<PgPool>,
    tenant_filter: Vec<Uuid>,
}

impl TenantAwareTriggerRepository {
    pub fn new(db: Arc<PgPool>, tenant_filter: Vec<Uuid>) -> Self {
        Self { db, tenant_filter }
    }

    /// Update the tenant filter for this repository
    pub async fn update_tenant_filter(&self, _tenant_filter: Vec<Uuid>) {
        // Since tenant_filter is not mutable, we would need to use Arc<RwLock>
        // For now, this is a no-op as the repository is created with specific tenants
        // In a real implementation, you'd want to make tenant_filter mutable
    }

    fn db_to_oz_trigger(&self, db_trigger: DbTrigger) -> Result<Trigger> {
        let mut trigger: Trigger = serde_json::from_value(db_trigger.configuration)
            .context("Failed to deserialize trigger configuration")?;

        // Ensure name matches
        trigger.name = db_trigger.name;

        Ok(trigger)
    }
}

#[async_trait]
impl TriggerRepositoryTrait for TenantAwareTriggerRepository {
    async fn new(_path: Option<&Path>) -> Result<Self, OzRepositoryError>
    where
        Self: Sized,
    {
        // This doesn't make sense for database-backed storage
        Err(OzRepositoryError::load_error(
            "Direct construction not supported - use TenantAwareTriggerRepository::new()",
            None,
            None,
        ))
    }

    async fn load_all(_path: Option<&Path>) -> Result<HashMap<String, Trigger>, OzRepositoryError> {
        // This doesn't make sense for database-backed storage
        Err(OzRepositoryError::load_error(
            "Static loading not supported - use instance methods",
            None,
            None,
        ))
    }

    fn get_all(&self) -> HashMap<String, Trigger> {
        execute_async(async {
            match self.get_all_internal().await {
                Ok(triggers) => triggers,
                Err(e) => {
                    tracing::error!("Failed to get triggers: {}", e);
                    HashMap::new()
                }
            }
        })
    }

    fn get(&self, name: &str) -> Option<Trigger> {
        execute_async(async {
            match self.get_internal(name).await {
                Ok(trigger) => trigger,
                Err(e) => {
                    tracing::error!("Failed to get trigger {}: {}", name, e);
                    None
                }
            }
        })
    }
}

impl TenantAwareTriggerRepository {
    /// Get triggers by monitor ID (not part of trait)
    pub fn get_by_monitor_id(&self, monitor_id: &str) -> Vec<Trigger> {
        execute_async(async {
            match self.get_by_monitor_id_internal(monitor_id).await {
                Ok(triggers) => triggers,
                Err(e) => {
                    tracing::error!("Failed to get triggers for monitor {}: {}", monitor_id, e);
                    Vec::new()
                }
            }
        })
    }

    async fn get_all_internal(&self) -> Result<HashMap<String, Trigger>, OzRepositoryError> {
        let triggers = sqlx::query_as!(
            DbTrigger,
            r#"
            SELECT 
                id, tenant_id, trigger_id, monitor_id, name, 
                type as "trigger_type!", 
                configuration, 
                is_active as "is_active!",
                created_at as "created_at!", 
                updated_at as "updated_at!"
            FROM tenant_triggers 
            WHERE tenant_id = ANY($1) AND is_active = true
            "#,
            &self.tenant_filter[..]
        )
        .fetch_all(&*self.db)
        .await
        .map_err(|e| to_oz_error(RepositoryError::from(e)))?;

        let mut result = HashMap::new();
        for db_trigger in triggers {
            let name = db_trigger.name.clone();
            let trigger = self
                .db_to_oz_trigger(db_trigger)
                .map_err(anyhow_to_oz_error)?;
            result.insert(name, trigger);
        }

        Ok(result)
    }

    async fn get_internal(&self, name: &str) -> Result<Option<Trigger>, OzRepositoryError> {
        let db_trigger = sqlx::query_as!(
            DbTrigger,
            r#"
            SELECT 
                id, tenant_id, trigger_id, monitor_id, name, 
                type as "trigger_type!", 
                configuration, 
                is_active as "is_active!",
                created_at as "created_at!", 
                updated_at as "updated_at!"
            FROM tenant_triggers 
            WHERE tenant_id = ANY($1) 
                AND name = $2 
                AND is_active = true
            LIMIT 1
            "#,
            &self.tenant_filter[..],
            name
        )
        .fetch_optional(&*self.db)
        .await
        .map_err(|e| to_oz_error(RepositoryError::from(e)))?;

        match db_trigger {
            Some(db_trigger) => Ok(Some(
                self.db_to_oz_trigger(db_trigger)
                    .map_err(anyhow_to_oz_error)?,
            )),
            None => Ok(None),
        }
    }

    async fn get_by_monitor_id_internal(
        &self,
        monitor_id: &str,
    ) -> Result<Vec<Trigger>, OzRepositoryError> {
        let triggers = sqlx::query_as!(
            DbTrigger,
            r#"
            SELECT 
                t.id, t.tenant_id, t.trigger_id, t.monitor_id, t.name, 
                t.type as "trigger_type!", 
                t.configuration, 
                t.is_active as "is_active!",
                t.created_at as "created_at!", 
                t.updated_at as "updated_at!"
            FROM tenant_triggers t
            JOIN tenant_monitors m ON t.monitor_id = m.id
            WHERE t.tenant_id = ANY($1) 
                AND m.monitor_id = $2
                AND t.is_active = true
            "#,
            &self.tenant_filter[..],
            monitor_id
        )
        .fetch_all(&*self.db)
        .await
        .map_err(|e| to_oz_error(RepositoryError::from(e)))?;

        let mut result = Vec::new();
        for db_trigger in triggers {
            let trigger = self
                .db_to_oz_trigger(db_trigger)
                .map_err(anyhow_to_oz_error)?;
            result.push(trigger);
        }

        Ok(result)
    }
}

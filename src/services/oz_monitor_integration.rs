//! OpenZeppelin Monitor Integration Module
//!
//! This module provides the integration layer between the orchestrator's multi-tenant
//! architecture and OpenZeppelin Monitor's core functionality. It wraps OZ Monitor's
//! services with tenant awareness and caching capabilities.

use anyhow::Result;
use dashmap::DashMap;
use sqlx::PgPool;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use tracing::{error, info, instrument};
use uuid::Uuid;

// Import OpenZeppelin Monitor types and services
use openzeppelin_monitor::{
    models::{
        BlockType, ContractSpec, EVMBlock, Monitor, MonitorMatch, Network, StellarBlock, Trigger,
    },
    repositories::{
        MonitorRepositoryTrait, NetworkRepositoryTrait, TriggerRepositoryTrait, TriggerService,
    },
    services::{
        blockchain::ClientPoolTrait,
        filter::FilterService,
        notification::NotificationService,
        trigger::{TriggerExecutionService, TriggerExecutionServiceTrait},
    },
};

use crate::repositories::{
    TenantAwareMonitorRepository, TenantAwareNetworkRepository, TenantAwareTriggerRepository,
};
use crate::services::cached_client_pool::CachedClientPool;

/// OpenZeppelin Monitor services wrapper with tenant awareness
pub struct OzMonitorServices {
    /// Filter service for evaluating blockchain data against monitor conditions
    filter_service: Arc<FilterService>,

    /// Trigger execution service for processing matches
    trigger_execution_service: Arc<TriggerExecutionService<TenantAwareTriggerRepository>>,

    /// Client pool for blockchain connections
    client_pool: Arc<CachedClientPool>,

    /// Tenant-aware repositories
    monitor_repo: Arc<TenantAwareMonitorRepository>,
    network_repo: Arc<TenantAwareNetworkRepository>,
    trigger_repo: Arc<TenantAwareTriggerRepository>,

    /// Cache for active monitors by tenant
    monitor_cache: Arc<DashMap<Uuid, HashMap<String, Monitor>>>,

    /// Cache for trigger scripts
    _trigger_script_cache: Arc<DashMap<String, String>>,

    /// Cache for contract specs
    contract_spec_cache: Arc<DashMap<String, ContractSpec>>,

    /// Database connection pool
    _db: Arc<PgPool>,

    /// Tenant IDs this service instance is responsible for
    tenant_ids: Vec<Uuid>,
}

impl OzMonitorServices {
    /// Create new OZ Monitor services instance with tenant awareness
    pub async fn new(
        db: Arc<PgPool>,
        tenant_ids: Vec<Uuid>,
        client_pool: Arc<CachedClientPool>,
    ) -> Result<Self> {
        info!(
            "Initializing OZ Monitor services for {} tenants",
            tenant_ids.len()
        );

        // Create tenant-aware repositories
        let monitor_repo = Arc::new(TenantAwareMonitorRepository::new(
            db.clone(),
            tenant_ids.clone(),
        ));
        let network_repo = Arc::new(TenantAwareNetworkRepository::new(
            db.clone(),
            tenant_ids.clone(),
        ));
        let trigger_repo = Arc::new(TenantAwareTriggerRepository::new(
            db.clone(),
            tenant_ids.clone(),
        ));

        // Initialize OZ Monitor services
        let filter_service = Arc::new(FilterService::new());

        // Create TriggerService from repository - dereference the Arc
        let trigger_service = TriggerService::new_with_repository((*trigger_repo).clone())
            .map_err(|e| anyhow::anyhow!("Failed to create trigger service: {}", e))?;

        // Create NotificationService
        let notification_service = NotificationService::new();

        let trigger_execution_service = Arc::new(TriggerExecutionService::new(
            trigger_service,
            notification_service,
        ));

        Ok(Self {
            filter_service,
            trigger_execution_service,
            client_pool,
            monitor_repo,
            network_repo,
            trigger_repo,
            monitor_cache: Arc::new(DashMap::new()),
            _trigger_script_cache: Arc::new(DashMap::new()),
            contract_spec_cache: Arc::new(DashMap::new()),
            _db: db,
            tenant_ids,
        })
    }

    /// Process a block for all tenant monitors
    #[instrument(skip(self, block))]
    pub async fn process_block<B>(
        &self,
        network: &Network,
        block: B,
        tenant_ids: &[Uuid],
    ) -> Result<Vec<TenantMonitorMatch>>
    where
        B: Into<BlockWrapper> + Clone,
    {
        let block_wrapper = block.into();
        let mut all_matches = Vec::new();

        // Process block for each tenant
        for tenant_id in tenant_ids {
            let context = self.get_tenant_context(*tenant_id).await?;

            match &block_wrapper {
                BlockWrapper::Ethereum(eth_block) => {
                    let matches = self
                        .process_ethereum_block(&context, network, eth_block)
                        .await?;
                    all_matches.extend(matches);
                }
                BlockWrapper::Stellar(stellar_block) => {
                    let matches = self
                        .process_stellar_block(&context, network, stellar_block)
                        .await?;
                    all_matches.extend(matches);
                }
            }
        }

        Ok(all_matches)
    }

    /// Process Ethereum block for a tenant
    async fn process_ethereum_block(
        &self,
        context: &TenantMonitorContext,
        network: &Network,
        block: &EVMBlock,
    ) -> Result<Vec<TenantMonitorMatch>> {
        let mut all_matches = Vec::new();

        // Get monitors for this network
        let monitors = context.get_monitors_for_network(&network.slug)?;
        let monitors_vec: Vec<Monitor> = monitors.values().cloned().collect();

        // Get the EVM client for this network
        let client = self
            .client_pool
            .get_evm_client(network)
            .await
            .map_err(|e| anyhow::anyhow!("Failed to get EVM client: {}", e))?;

        // Convert to BlockType for the filter service
        let block_type = BlockType::EVM(Box::new(block.clone()));

        // Get contract specs for this tenant
        let contract_specs = self
            .get_contract_specs_for_monitors(&monitors_vec, network)
            .await?;

        // Use OZ Monitor's filter service to process the entire block
        let filter_results = self
            .filter_service
            .filter_block(
                &*client,
                network,
                &block_type,
                &monitors_vec,
                Some(&contract_specs),
            )
            .await
            .map_err(|e| anyhow::anyhow!("Filter service error: {}", e))?;

        // Process each match
        for monitor_match in filter_results {
            // Find which monitor produced this match
            let monitor_address = match &monitor_match {
                MonitorMatch::EVM(evm_match) => {
                    match &evm_match.transaction.to {
                        Some(addr) => addr,
                        None => continue, // Skip contract creation transactions
                    }
                }
                MonitorMatch::Stellar(_) => {
                    // Stellar matches don't have a simple address field
                    continue;
                }
            };

            if let Some((monitor_name, monitor)) = monitors.iter().find(|(_, m)| {
                // Match based on monitor configuration
                m.addresses.iter().any(|addr| {
                    // Compare addresses as strings
                    format!("{:?}", monitor_address).eq_ignore_ascii_case(&addr.address)
                })
            }) {
                // Check trigger conditions
                if self
                    .evaluate_trigger_conditions(monitor, &monitor_match)
                    .await?
                {
                    all_matches.push(TenantMonitorMatch {
                        tenant_id: context.tenant_id,
                        monitor_name: monitor_name.clone(),
                        monitor_match,
                    });
                }
            }
        }

        Ok(all_matches)
    }

    /// Process Stellar block for a tenant
    async fn process_stellar_block(
        &self,
        context: &TenantMonitorContext,
        network: &Network,
        block: &StellarBlock,
    ) -> Result<Vec<TenantMonitorMatch>> {
        let mut all_matches = Vec::new();

        // Get monitors for this network
        let monitors = context.get_monitors_for_network(&network.slug)?;
        let monitors_vec: Vec<Monitor> = monitors.values().cloned().collect();

        // Get the Stellar client for this network
        let client = self
            .client_pool
            .get_stellar_client(network)
            .await
            .map_err(|e| anyhow::anyhow!("Failed to get Stellar client: {}", e))?;

        // Convert to BlockType for the filter service
        let block_type = BlockType::Stellar(Box::new(block.clone()));

        // Get contract specs for this tenant
        let contract_specs = self
            .get_contract_specs_for_monitors(&monitors_vec, network)
            .await?;

        // Use OZ Monitor's filter service to process the entire block
        let filter_results = self
            .filter_service
            .filter_block(
                &*client,
                network,
                &block_type,
                &monitors_vec,
                Some(&contract_specs),
            )
            .await
            .map_err(|e| anyhow::anyhow!("Filter service error: {}", e))?;

        // Process each match
        for monitor_match in filter_results {
            // For Stellar, extract the contract address from the matched_on_args
            let contract_address = match &monitor_match {
                MonitorMatch::Stellar(stellar_match) => {
                    // Try to get contract address from matched function arguments
                    if let Some(matched_args) = &stellar_match.matched_on_args {
                        if let Some(_functions) = &matched_args.functions {
                            // For Stellar, the contract address is usually part of the transaction
                            // We need to extract it from the transaction operations
                            self.extract_stellar_contract_address(stellar_match)?
                        } else {
                            continue; // No function matches
                        }
                    } else {
                        continue; // No matched args
                    }
                }
                MonitorMatch::EVM(_) => {
                    continue; // This is Stellar block processing
                }
            };

            // Find which monitor produced this match
            if let Some((monitor_name, monitor)) = monitors.iter().find(|(_, m)| {
                // Match based on monitor configuration
                m.addresses.iter().any(|addr| {
                    // Compare Stellar addresses (case-insensitive)
                    addr.address.eq_ignore_ascii_case(&contract_address)
                })
            }) {
                // Check trigger conditions
                if self
                    .evaluate_trigger_conditions(monitor, &monitor_match)
                    .await?
                {
                    all_matches.push(TenantMonitorMatch {
                        tenant_id: context.tenant_id,
                        monitor_name: monitor_name.clone(),
                        monitor_match,
                    });
                }
            }
        }

        Ok(all_matches)
    }

    /// Extract contract address from Stellar monitor match
    fn extract_stellar_contract_address(
        &self,
        stellar_match: &openzeppelin_monitor::models::StellarMonitorMatch,
    ) -> Result<String> {
        // First, check if we have a contract address in the monitor configuration
        if let Some(addr) = stellar_match.monitor.addresses.first() {
            return Ok(addr.address.clone());
        }

        // Try to extract from transaction envelope
        if let Some(envelope_json) = &stellar_match.transaction.envelope_json {
            if let Some(tx) = envelope_json.get("tx") {
                if let Some(operations) = tx.get("operations") {
                    if let Some(ops_array) = operations.as_array() {
                        for op in ops_array {
                            if let Some(op_type) = op.get("type").and_then(|t| t.as_str()) {
                                if op_type == "invokeHostFunction" {
                                    // For contract invocations, the contract address might be in the function parameters
                                    if let Some(host_func) = op.get("hostFunction") {
                                        if let Some(contract_id) =
                                            host_func.get("contractId").and_then(|c| c.as_str())
                                        {
                                            return Ok(contract_id.to_string());
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        Err(anyhow::anyhow!(
            "Could not extract contract address from Stellar transaction"
        ))
    }

    /// Evaluate trigger conditions for a monitor match
    async fn evaluate_trigger_conditions(
        &self,
        monitor: &Monitor,
        monitor_match: &MonitorMatch,
    ) -> Result<bool> {
        // If no trigger conditions, include the match
        if monitor.trigger_conditions.is_empty() {
            return Ok(true);
        }

        // Evaluate all trigger conditions - ALL must return true for the match to be included
        for condition in &monitor.trigger_conditions {
            // Check if we have the script cached
            let script_content =
                if let Some(script) = self._trigger_script_cache.get(&condition.script_path) {
                    script.clone()
                } else {
                    // Load from database using script_path as the script name
                    match self.load_script_from_database(&condition.script_path).await {
                        Ok(content) => {
                            self._trigger_script_cache
                                .insert(condition.script_path.clone(), content.clone());
                            content
                        }
                        Err(e) => {
                            error!(
                                "Failed to load trigger script {}: {}. Including match by default.",
                                condition.script_path, e
                            );
                            // If we can't load the script, include the match by default for safety
                            return Ok(true);
                        }
                    }
                };

            // Create script executor based on language
            use openzeppelin_monitor::services::trigger::ScriptExecutorFactory;

            let executor = ScriptExecutorFactory::create(&condition.language, &script_content);

            // Execute the script with timeout
            let timeout_ms = condition.timeout_ms; // timeout_ms is already a u32 in TriggerCondition

            match executor
                .execute(
                    monitor_match.clone(),
                    &timeout_ms,
                    condition.arguments.as_deref(),
                    false, // Not from custom notification
                )
                .await
            {
                Ok(result) => {
                    if !result {
                        // If any condition returns false, exclude the match
                        return Ok(false);
                    }
                }
                Err(e) => {
                    error!(
                        "Error executing trigger condition script {}: {}. Including match by default.",
                        condition.script_path, e
                    );
                    // On error, include the match by default for safety
                    return Ok(true);
                }
            }
        }

        // All conditions returned true
        Ok(true)
    }

    /// Execute triggers for a monitor match
    pub async fn execute_triggers(&self, tenant_match: &TenantMonitorMatch) -> Result<()> {
        let context = self.get_tenant_context(tenant_match.tenant_id).await?;
        let monitor = context.get_monitor(&tenant_match.monitor_name)?;

        // Prepare trigger scripts (empty for now)
        let trigger_scripts = HashMap::new();

        // Prepare variables for trigger execution
        let mut variables = HashMap::new();
        variables.insert("monitor_name".to_string(), monitor.name.clone());
        variables.insert(
            "network".to_string(),
            match &tenant_match.monitor_match {
                MonitorMatch::EVM(evm_match) => evm_match.network_slug.clone(),
                MonitorMatch::Stellar(stellar_match) => stellar_match.network_slug.clone(),
            },
        );

        // Execute triggers
        let result = self
            .trigger_execution_service
            .execute(
                &monitor.triggers,
                variables,
                &tenant_match.monitor_match,
                &trigger_scripts,
            )
            .await;

        if let Err(e) = result {
            error!(
                "Failed to execute triggers for monitor {} for tenant {}: {}",
                monitor.name, tenant_match.tenant_id, e
            );
        }

        Ok(())
    }

    /// Get or create tenant context
    async fn get_tenant_context(&self, tenant_id: Uuid) -> Result<TenantMonitorContext> {
        // Check cache first
        if let Some(monitors) = self.monitor_cache.get(&tenant_id) {
            return Ok(TenantMonitorContext {
                tenant_id,
                monitors: monitors.clone(),
                networks: self.load_tenant_networks(tenant_id).await?,
                triggers: self.load_tenant_triggers(tenant_id).await?,
            });
        }

        // Load from database
        let monitors = self.load_tenant_monitors(tenant_id).await?;
        let networks = self.load_tenant_networks(tenant_id).await?;
        let triggers = self.load_tenant_triggers(tenant_id).await?;

        // Cache the monitors
        self.monitor_cache.insert(tenant_id, monitors.clone());

        Ok(TenantMonitorContext {
            tenant_id,
            monitors,
            networks,
            triggers,
        })
    }

    /// Load monitors for a tenant
    async fn load_tenant_monitors(&self, tenant_id: Uuid) -> Result<HashMap<String, Monitor>> {
        // Update repository tenant filter
        self.monitor_repo
            .update_tenant_filter(vec![tenant_id])
            .await;

        // Load all monitors
        Ok(self.monitor_repo.get_all())
    }

    /// Load networks for a tenant
    async fn load_tenant_networks(&self, tenant_id: Uuid) -> Result<HashMap<String, Network>> {
        self.network_repo
            .update_tenant_filter(vec![tenant_id])
            .await;
        Ok(self.network_repo.get_all())
    }

    /// Load triggers for a tenant
    async fn load_tenant_triggers(&self, tenant_id: Uuid) -> Result<HashMap<String, Trigger>> {
        self.trigger_repo
            .update_tenant_filter(vec![tenant_id])
            .await;
        Ok(self.trigger_repo.get_all())
    }

    /// Load script from database by name
    async fn load_script_from_database(&self, script_name: &str) -> Result<String> {
        // Extract script name from path if it's a full path
        let name = if script_name.contains('/') {
            script_name
                .split('/')
                .last()
                .unwrap_or(script_name)
                .trim_end_matches(".py")
                .trim_end_matches(".js")
                .trim_end_matches(".sh")
        } else {
            script_name
        };

        // Query database for script
        #[derive(sqlx::FromRow)]
        struct ScriptRow {
            content: String,
        }

        let result = sqlx::query_as::<_, ScriptRow>(
            r#"
            SELECT content
            FROM trigger_scripts
            WHERE name = $1 
                AND tenant_id = ANY($2)
                AND is_active = true
            LIMIT 1
            "#,
        )
        .bind(name)
        .bind(self.tenant_filter())
        .fetch_optional(&*self._db)
        .await?;

        match result {
            Some(row) => Ok(row.content),
            None => {
                // Fallback to filesystem for backward compatibility
                // This allows gradual migration of scripts to database
                match tokio::fs::read_to_string(script_name).await {
                    Ok(content) => {
                        info!(
                            "Script {} not found in database, loaded from filesystem. Consider migrating to database.",
                            script_name
                        );
                        Ok(content)
                    }
                    Err(e) => Err(anyhow::anyhow!(
                        "Script {} not found in database or filesystem: {}",
                        name,
                        e
                    )),
                }
            }
        }
    }

    /// Get tenant filter
    fn tenant_filter(&self) -> &[Uuid] {
        &self.tenant_ids
    }

    /// Reload configuration for specific tenants
    pub async fn reload_configurations(&self, tenant_ids: &[Uuid]) -> Result<()> {
        info!("Reloading configuration for {} tenants", tenant_ids.len());

        // Clear cache for these tenants
        for tenant_id in tenant_ids {
            self.monitor_cache.remove(tenant_id);
        }

        // Update repository filters
        self.monitor_repo
            .update_tenant_filter(tenant_ids.to_vec())
            .await;
        self.network_repo
            .update_tenant_filter(tenant_ids.to_vec())
            .await;
        self.trigger_repo
            .update_tenant_filter(tenant_ids.to_vec())
            .await;

        Ok(())
    }

    /// Get active networks across all assigned tenants
    pub async fn get_active_networks(&self) -> Result<HashSet<String>> {
        let mut networks = HashSet::new();

        // Get all monitors across tenants
        let all_monitors = self.monitor_repo.get_all();

        // Extract unique networks
        for (_, monitor) in all_monitors {
            networks.extend(monitor.networks);
        }

        Ok(networks)
    }

    /// Get client pool reference
    pub fn client_pool(&self) -> Arc<CachedClientPool> {
        self.client_pool.clone()
    }

    /// Get contract specs for a set of monitors
    async fn get_contract_specs_for_monitors(
        &self,
        monitors: &[Monitor],
        network: &Network,
    ) -> Result<Vec<(String, ContractSpec)>> {
        let mut specs = Vec::new();

        // Collect contract specs from monitor configurations
        for monitor in monitors {
            for address in &monitor.addresses {
                if let Some(spec) = &address.contract_spec {
                    // Check cache first
                    let cache_key = format!("{}:{}", network.slug, address.address);
                    if let Some(cached_spec) = self.contract_spec_cache.get(&cache_key) {
                        specs.push((address.address.clone(), cached_spec.clone()));
                    } else {
                        // Cache the spec
                        self.contract_spec_cache.insert(cache_key, spec.clone());
                        specs.push((address.address.clone(), spec.clone()));
                    }
                }
            }
        }

        Ok(specs)
    }
}

/// Tenant-specific monitor context
pub struct TenantMonitorContext {
    pub tenant_id: Uuid,
    pub monitors: HashMap<String, Monitor>,
    pub networks: HashMap<String, Network>,
    pub triggers: HashMap<String, Trigger>,
}

impl TenantMonitorContext {
    /// Get monitors configured for a specific network
    pub fn get_monitors_for_network(&self, network_slug: &str) -> Result<HashMap<String, Monitor>> {
        let mut network_monitors = HashMap::new();

        for (name, monitor) in &self.monitors {
            if monitor.networks.contains(&network_slug.to_string()) {
                network_monitors.insert(name.clone(), monitor.clone());
            }
        }

        Ok(network_monitors)
    }

    /// Get a specific monitor by name
    pub fn get_monitor(&self, name: &str) -> Result<Monitor> {
        self.monitors
            .get(name)
            .cloned()
            .ok_or_else(|| anyhow::anyhow!("Monitor {} not found", name))
    }
}

/// Monitor match with tenant information
#[derive(Debug, Clone)]
pub struct TenantMonitorMatch {
    pub tenant_id: Uuid,
    pub monitor_name: String,
    pub monitor_match: MonitorMatch,
}

/// Block wrapper to handle different blockchain types
#[derive(Debug, Clone)]
pub enum BlockWrapper {
    Ethereum(EVMBlock),
    Stellar(StellarBlock),
}

impl From<EVMBlock> for BlockWrapper {
    fn from(block: EVMBlock) -> Self {
        BlockWrapper::Ethereum(block)
    }
}

impl From<StellarBlock> for BlockWrapper {
    fn from(block: StellarBlock) -> Self {
        BlockWrapper::Stellar(block)
    }
}

impl From<BlockType> for BlockWrapper {
    fn from(block: BlockType) -> Self {
        match block {
            BlockType::EVM(eth_block) => BlockWrapper::Ethereum(*eth_block),
            BlockType::Stellar(stellar_block) => BlockWrapper::Stellar(*stellar_block),
        }
    }
}

#[cfg(test)]
mod tests {
    #[tokio::test]
    async fn test_oz_monitor_services_creation() {
        // Test service creation
        // This would require mock implementations
    }

    #[tokio::test]
    async fn test_tenant_context_loading() {
        // Test tenant context loading and caching
    }

    #[tokio::test]
    async fn test_block_processing() {
        // Test block processing for different blockchain types
    }
}

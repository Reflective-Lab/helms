use std::sync::{Arc, RwLock};

use crm_kernel::{CrmKernel, DomainEvent, KernelError, KernelResult};
use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SurrealStoreConfig {
    pub endpoint: String,
    pub namespace: String,
    pub database: String,
    pub username: Option<String>,
    pub password: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LanceStoreConfig {
    pub uri: String,
    pub embedding_dim: usize,
    pub table_name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RecordStoreConfig {
    Memory,
    Surreal(SurrealStoreConfig),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum VectorStoreConfig {
    Disabled,
    LanceDb(LanceStoreConfig),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum UsageSourceKind {
    ProductEvents,
    MarketingSite,
    DocumentationSite,
    ExternalWebsite,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UsageIngestionConfig {
    pub source_name: String,
    pub source_kind: UsageSourceKind,
    pub site_root: Option<String>,
    pub ingest_url: Option<String>,
    pub aggregates_url: Option<String>,
    pub enabled: bool,
    pub workspace_id: Option<String>,
    pub correlation_keys: Vec<String>,
    pub event_types: Vec<String>,
    pub analytics_vendors: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuntimeModuleConfig {
    pub name: String,
    pub purpose: String,
    pub enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConvergeFeatureConfig {
    pub analytics_enabled: bool,
    pub optimization_enabled: bool,
    pub llm_enabled: bool,
    pub runtime_modules: Vec<RuntimeModuleConfig>,
}

impl Default for ConvergeFeatureConfig {
    fn default() -> Self {
        Self {
            analytics_enabled: true,
            optimization_enabled: true,
            llm_enabled: true,
            runtime_modules: vec![
                RuntimeModuleConfig {
                    name: "linkedin-scan".to_string(),
                    purpose: "Governed LinkedIn profile and company research".to_string(),
                    enabled: true,
                },
                RuntimeModuleConfig {
                    name: "website-usage-ingest".to_string(),
                    purpose: "First-party marketing and product behavior ingestion".to_string(),
                    enabled: true,
                },
                RuntimeModuleConfig {
                    name: "lead-routing".to_string(),
                    purpose: "Optimization-backed queueing and prioritization".to_string(),
                    enabled: true,
                },
                RuntimeModuleConfig {
                    name: "account-fit-scoring".to_string(),
                    purpose: "Analytics or ML scoring over CRM and usage signals".to_string(),
                    enabled: true,
                },
            ],
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    pub record_store: RecordStoreConfig,
    pub vector_store: VectorStoreConfig,
    pub usage_ingestion: Vec<UsageIngestionConfig>,
    pub converge: ConvergeFeatureConfig,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            record_store: RecordStoreConfig::Memory,
            vector_store: VectorStoreConfig::Disabled,
            usage_ingestion: vec![UsageIngestionConfig {
                source_name: "www.converge.zone".to_string(),
                source_kind: UsageSourceKind::MarketingSite,
                site_root: Some("/Users/kpernyer/dev/brand/www.converge.zone".to_string()),
                ingest_url: Some(
                    "https://us-central1-converge-369ad.cloudfunctions.net/analyticsIngest"
                        .to_string(),
                ),
                aggregates_url: Some(
                    "https://us-central1-converge-369ad.cloudfunctions.net/analyticsAggregates"
                        .to_string(),
                ),
                enabled: true,
                workspace_id: Some("converge".to_string()),
                correlation_keys: vec![
                    "anonymous_id".to_string(),
                    "session_id".to_string(),
                    "email".to_string(),
                    "company_domain".to_string(),
                ],
                event_types: vec![
                    "session_start".to_string(),
                    "page_view".to_string(),
                    "page_scroll_milestone".to_string(),
                    "page_summary".to_string(),
                    "link_click".to_string(),
                ],
                analytics_vendors: vec!["firebase-functions".to_string()],
            }],
            converge: ConvergeFeatureConfig::default(),
        }
    }
}

#[derive(Debug, Error)]
pub enum StorageError {
    #[error("storage lock poisoned")]
    LockPoisoned,
    #[error(transparent)]
    Kernel(#[from] KernelError),
}

pub type StorageResult<T> = Result<T, StorageError>;

#[derive(Debug)]
pub struct StoreWriteResult<T> {
    pub value: T,
    pub events: Vec<DomainEvent>,
}

pub trait KernelStore: Clone + Send + Sync + 'static {
    fn read<R, F>(&self, f: F) -> StorageResult<R>
    where
        F: FnOnce(&CrmKernel) -> R;

    fn write_with_events<R, F>(&self, f: F) -> StorageResult<StoreWriteResult<R>>
    where
        F: FnOnce(&mut CrmKernel) -> KernelResult<R>;

    fn write<R, F>(&self, f: F) -> StorageResult<R>
    where
        F: FnOnce(&mut CrmKernel) -> KernelResult<R>,
    {
        self.write_with_events(f).map(|result| result.value)
    }
}

#[derive(Clone)]
pub struct InMemoryKernelStore {
    kernel: Arc<RwLock<CrmKernel>>,
    pub config: AppConfig,
}

impl std::fmt::Debug for InMemoryKernelStore {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("InMemoryKernelStore")
            .field("config", &self.config)
            .finish_non_exhaustive()
    }
}

impl InMemoryKernelStore {
    #[must_use]
    pub fn new(config: AppConfig) -> Self {
        Self {
            kernel: Arc::new(RwLock::new(CrmKernel::default())),
            config,
        }
    }

    #[must_use]
    pub fn default_local() -> Self {
        Self::new(AppConfig::default())
    }

    pub fn read<R>(&self, f: impl FnOnce(&CrmKernel) -> R) -> StorageResult<R> {
        let kernel = self.kernel.read().map_err(|_| StorageError::LockPoisoned)?;
        Ok(f(&kernel))
    }

    pub fn write<R>(&self, f: impl FnOnce(&mut CrmKernel) -> KernelResult<R>) -> StorageResult<R> {
        self.write_with_events(f).map(|result| result.value)
    }

    pub fn write_with_events<R>(
        &self,
        f: impl FnOnce(&mut CrmKernel) -> KernelResult<R>,
    ) -> StorageResult<StoreWriteResult<R>> {
        let mut kernel = self
            .kernel
            .write()
            .map_err(|_| StorageError::LockPoisoned)?;
        let result = f(&mut kernel);
        let events = kernel.drain_events();
        let value = result?;
        Ok(StoreWriteResult { value, events })
    }
}

impl KernelStore for InMemoryKernelStore {
    fn read<R, F>(&self, f: F) -> StorageResult<R>
    where
        F: FnOnce(&CrmKernel) -> R,
    {
        InMemoryKernelStore::read(self, f)
    }

    fn write_with_events<R, F>(&self, f: F) -> StorageResult<StoreWriteResult<R>>
    where
        F: FnOnce(&mut CrmKernel) -> KernelResult<R>,
    {
        InMemoryKernelStore::write_with_events(self, f)
    }
}

#[must_use]
pub fn runtime_modules_for_local_crm() -> Vec<RuntimeModuleConfig> {
    ConvergeFeatureConfig::default().runtime_modules
}

#[cfg(test)]
mod tests {
    use crm_kernel::{Actor, ActorKind, DomainEvent, OrganizationLifecycle, OrganizationUpsert};

    use super::InMemoryKernelStore;

    fn human() -> Actor {
        Actor {
            actor_id: "user-1".to_string(),
            display_name: "Kenneth".to_string(),
            kind: ActorKind::Human,
        }
    }

    #[test]
    fn write_with_events_returns_emitted_domain_events() {
        let store = InMemoryKernelStore::default_local();
        let actor = human();

        let result = store
            .write_with_events(|kernel| {
                kernel.upsert_organization(
                    OrganizationUpsert {
                        organization_id: None,
                        name: "Aprio".to_string(),
                        external_key: None,
                        website: None,
                        industry: None,
                        lifecycle: OrganizationLifecycle::Prospect,
                        owner_user_id: None,
                        tags: vec![],
                    },
                    actor.clone(),
                )
            })
            .expect("write should succeed");

        assert_eq!(result.events.len(), 3);
        assert!(matches!(
            &result.events[0],
            DomainEvent::OrganizationUpserted { .. }
        ));
    }
}

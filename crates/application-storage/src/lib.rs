use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, Mutex, RwLock};

use application_kernel::{CrmKernel, DomainEvent, KernelError, KernelResult};
use converge_core::experience_store::{EventQuery, ExperienceEventEnvelope};
use converge_core::traits::{ContextStore, StoreError};
use converge_core::{ContextSnapshot, ContextState as Context, UserExperienceEventEnvelope};
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use surrealdb::Surreal;
use surrealdb::engine::any::{self, Any};
use surrealdb::opt::auth::Root;
use thiserror::Error;
use tokio::runtime::{Builder as RuntimeBuilder, Handle, Runtime};

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
pub enum ContextSnapshotStoreConfig {
    Memory,
    Surreal(SurrealStoreConfig),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ExperienceLedgerStoreConfig {
    Memory,
    Surreal(SurrealStoreConfig),
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
    pub context_store: ContextSnapshotStoreConfig,
    pub experience_store: ExperienceLedgerStoreConfig,
    pub usage_ingestion: Vec<UsageIngestionConfig>,
    pub converge: ConvergeFeatureConfig,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            record_store: RecordStoreConfig::Memory,
            vector_store: VectorStoreConfig::Disabled,
            context_store: ContextSnapshotStoreConfig::Memory,
            experience_store: ExperienceLedgerStoreConfig::Memory,
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
    #[error("storage connection failed: {backend} — {message}")]
    ConnectionFailed { backend: String, message: String },
    #[error("storage serialization failed: {message}")]
    SerializationFailed { message: String },
    #[error("storage timeout: {operation}")]
    Timeout { operation: String },
    #[error("runtime store failure: {message}")]
    RuntimeStore { message: String },
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

#[derive(Debug, Clone)]
pub enum AppKernelStore {
    Memory(InMemoryKernelStore),
    Surreal(SurrealDbKernelStore),
}

#[derive(Debug, Clone)]
pub enum AppContextStore {
    Memory(InMemoryContextStore),
    Surreal(SurrealDbContextStore),
}

#[derive(Debug, Clone)]
pub enum AppExperienceStore {
    Memory(InMemoryExperienceStoreAdapter),
}

#[derive(Debug, Clone)]
pub struct AppRuntimeStores {
    pub context: AppContextStore,
    pub experience: AppExperienceStore,
}

#[derive(Debug, Clone, Default)]
pub struct InMemoryContextStore {
    contexts: Arc<RwLock<std::collections::HashMap<String, Context>>>,
}

#[derive(Clone)]
pub struct SurrealDbContextStore {
    db: Arc<Surreal<Any>>,
    write_lock: Arc<Mutex<()>>,
    runtime: Option<Arc<Runtime>>,
}

impl Default for AppContextStore {
    fn default() -> Self {
        Self::Memory(InMemoryContextStore::default())
    }
}

impl Default for AppExperienceStore {
    fn default() -> Self {
        Self::Memory(InMemoryExperienceStoreAdapter::default())
    }
}

impl Default for AppRuntimeStores {
    fn default() -> Self {
        Self {
            context: AppContextStore::default(),
            experience: AppExperienceStore::default(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct InMemoryExperienceStoreAdapter {
    inner: Arc<converge_experience::InMemoryExperienceStore>,
}

#[derive(Clone)]
pub struct SurrealDbKernelStore {
    db: Arc<Surreal<Any>>,
    write_lock: Arc<Mutex<()>>,
    runtime: Option<Arc<Runtime>>,
    pub config: AppConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct KernelSnapshotDocument {
    kernel: CrmKernel,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ContextSnapshotDocument {
    #[serde(default)]
    snapshot: Option<ContextSnapshot>,
    // Earlier local builds persisted either an opaque JSON context blob or a
    // hand-rolled fact snapshot. Keep the legacy field for tolerant reads, but
    // only restore from Converge-owned ContextSnapshot envelopes.
    #[serde(default, alias = "context", alias = "context_json")]
    legacy_context_json: Option<JsonValue>,
}

const KERNEL_SNAPSHOT_TABLE: &str = "crm_kernel";
const KERNEL_SNAPSHOT_ID: &str = "default";
const CONTEXT_SNAPSHOT_TABLE: &str = "converge_context";

impl ContextSnapshotDocument {
    fn from_runtime(context: &Context) -> Self {
        Self {
            snapshot: Some(context.snapshot()),
            legacy_context_json: None,
        }
    }

    fn into_runtime(self) -> StorageResult<Option<Context>> {
        match self.snapshot {
            Some(snapshot) => Context::from_snapshot(snapshot).map(Some).map_err(|error| {
                StorageError::SerializationFailed {
                    message: format!("invalid Converge context snapshot: {error}"),
                }
            }),
            None if self.legacy_context_json.is_some() => Err(StorageError::SerializationFailed {
                message: "legacy context document cannot be safely restored after Converge v3.8; persist a Converge ContextSnapshot instead".to_string(),
            }),
            None => Ok(None),
        }
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
        let mut snapshot = kernel.clone();
        let value = f(&mut snapshot)?;
        let events = snapshot.drain_events();
        *kernel = snapshot;
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

impl InMemoryContextStore {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    fn load_sync(&self, scope_id: &str) -> Result<Option<Context>, StoreError> {
        let contexts = self.contexts.read().map_err(|_| StoreError::Internal {
            message: "context store lock poisoned".to_string(),
        })?;
        Ok(contexts.get(scope_id).cloned())
    }

    fn save_sync(&self, scope_id: &str, context: &Context) -> Result<(), StoreError> {
        let mut contexts = self.contexts.write().map_err(|_| StoreError::Internal {
            message: "context store lock poisoned".to_string(),
        })?;
        contexts.insert(scope_id.to_string(), context.clone());
        Ok(())
    }
}

impl ContextStore for InMemoryContextStore {
    type LoadFut<'a>
        = Pin<Box<dyn Future<Output = Result<Option<Context>, StoreError>> + Send + 'a>>
    where
        Self: 'a;
    type SaveFut<'a>
        = Pin<Box<dyn Future<Output = Result<(), StoreError>> + Send + 'a>>
    where
        Self: 'a;

    fn load_context<'a>(&'a self, scope_id: &'a str) -> Self::LoadFut<'a> {
        Box::pin(async move { self.load_sync(scope_id) })
    }

    fn save_context<'a>(&'a self, scope_id: &'a str, context: &'a Context) -> Self::SaveFut<'a> {
        Box::pin(async move { self.save_sync(scope_id, context) })
    }
}

impl InMemoryExperienceStoreAdapter {
    #[must_use]
    pub fn new() -> Self {
        Self {
            inner: Arc::new(converge_experience::InMemoryExperienceStore::new()),
        }
    }

    #[allow(deprecated)]
    fn append_sync(&self, events: &[ExperienceEventEnvelope]) -> Result<(), StoreError> {
        <dyn converge_core::ExperienceStore>::append_events(self.inner.as_ref(), events)
            .map_err(map_legacy_experience_error)
    }

    #[allow(deprecated)]
    fn query_sync(&self, query: &EventQuery) -> Result<Vec<ExperienceEventEnvelope>, StoreError> {
        <dyn converge_core::ExperienceStore>::query_events(self.inner.as_ref(), query)
            .map_err(map_legacy_experience_error)
    }

    fn append_user_sync(&self, event: UserExperienceEventEnvelope) -> Result<(), StoreError> {
        <dyn converge_core::ExperienceStore>::append_user_event(self.inner.as_ref(), event)
            .map_err(map_legacy_experience_error)
    }

    /// Expose the underlying `ExperienceStore` handle so consumers (e.g.
    /// `PlanningPriorAgent`) can run recall queries against the same ledger
    /// the engine writes to.
    #[must_use]
    pub fn handle(&self) -> Arc<converge_experience::InMemoryExperienceStore> {
        Arc::clone(&self.inner)
    }
}

impl Default for InMemoryExperienceStoreAdapter {
    fn default() -> Self {
        Self::new()
    }
}

impl InMemoryExperienceStoreAdapter {
    fn append<'a>(
        &'a self,
        events: &'a [ExperienceEventEnvelope],
    ) -> Pin<Box<dyn Future<Output = Result<(), StoreError>> + Send + 'a>> {
        Box::pin(async move { self.append_sync(events) })
    }

    fn query<'a>(
        &'a self,
        query: &'a EventQuery,
    ) -> Pin<Box<dyn Future<Output = Result<Vec<ExperienceEventEnvelope>, StoreError>> + Send + 'a>>
    {
        Box::pin(async move { self.query_sync(query) })
    }
}

impl SurrealDbContextStore {
    pub async fn connect(config: &ContextSnapshotStoreConfig) -> StorageResult<Self> {
        let record_store = match config {
            ContextSnapshotStoreConfig::Surreal(record_store) => record_store.clone(),
            ContextSnapshotStoreConfig::Memory => {
                return Err(StorageError::ConnectionFailed {
                    backend: "surrealdb-context".to_string(),
                    message: "context_store must be configured for surreal".to_string(),
                });
            }
        };

        let db = any::connect(record_store.endpoint.as_str())
            .await
            .map_err(|error| StorageError::ConnectionFailed {
                backend: "surrealdb-context".to_string(),
                message: error.to_string(),
            })?;

        if let (Some(username), Some(password)) = (
            record_store.username.as_deref(),
            record_store.password.as_deref(),
        ) {
            db.signin(Root {
                username: username.to_string(),
                password: password.to_string(),
            })
            .await
            .map_err(|error| StorageError::ConnectionFailed {
                backend: "surrealdb-context".to_string(),
                message: error.to_string(),
            })?;
        }

        db.use_ns(record_store.namespace.as_str())
            .use_db(record_store.database.as_str())
            .await
            .map_err(|error| StorageError::ConnectionFailed {
                backend: "surrealdb-context".to_string(),
                message: error.to_string(),
            })?;

        Ok(Self {
            db: Arc::new(db),
            write_lock: Arc::new(Mutex::new(())),
            runtime: None,
        })
    }

    fn load_sync(&self, scope_id: &str) -> Result<Option<Context>, StoreError> {
        self.block_on(self.load_context_async(scope_id))
            .map_err(map_storage_to_store_error)
    }

    fn save_sync(&self, scope_id: &str, context: &Context) -> Result<(), StoreError> {
        self.block_on(self.save_context_async(scope_id, context))
            .map_err(map_storage_to_store_error)
    }

    async fn load_context_async(&self, scope_id: &str) -> StorageResult<Option<Context>> {
        let document: Option<JsonValue> =
            match self.db.select((CONTEXT_SNAPSHOT_TABLE, scope_id)).await {
                Ok(document) => document,
                Err(error) => {
                    let message = error.to_string();
                    if message.contains(&format!("table '{CONTEXT_SNAPSHOT_TABLE}' does not exist"))
                    {
                        None
                    } else {
                        return Err(StorageError::ConnectionFailed {
                            backend: "surrealdb-context".to_string(),
                            message,
                        });
                    }
                }
            };
        match document {
            Some(document) => {
                let document = serde_json::from_value::<ContextSnapshotDocument>(document)
                    .map_err(|error| StorageError::SerializationFailed {
                        message: error.to_string(),
                    })?;

                document.into_runtime()
            }
            None => Ok(None),
        }
    }

    async fn save_context_async(&self, scope_id: &str, context: &Context) -> StorageResult<()> {
        let _guard = self
            .write_lock
            .lock()
            .map_err(|_| StorageError::LockPoisoned)?;
        let document = serde_json::to_value(ContextSnapshotDocument::from_runtime(context))
            .map_err(|error| StorageError::SerializationFailed {
                message: error.to_string(),
            })?;

        let _: Option<JsonValue> = self
            .db
            .upsert((CONTEXT_SNAPSHOT_TABLE, scope_id))
            .content(document)
            .await
            .map_err(|error| StorageError::SerializationFailed {
                message: error.to_string(),
            })?;
        Ok(())
    }

    fn block_on<T>(&self, future: impl Future<Output = StorageResult<T>>) -> StorageResult<T> {
        if let Some(runtime) = &self.runtime {
            runtime.block_on(future)
        } else {
            block_on_storage(future)
        }
    }
}

impl ContextStore for SurrealDbContextStore {
    type LoadFut<'a>
        = Pin<Box<dyn Future<Output = Result<Option<Context>, StoreError>> + Send + 'a>>
    where
        Self: 'a;
    type SaveFut<'a>
        = Pin<Box<dyn Future<Output = Result<(), StoreError>> + Send + 'a>>
    where
        Self: 'a;

    fn load_context<'a>(&'a self, scope_id: &'a str) -> Self::LoadFut<'a> {
        Box::pin(async move { self.load_sync(scope_id) })
    }

    fn save_context<'a>(&'a self, scope_id: &'a str, context: &'a Context) -> Self::SaveFut<'a> {
        Box::pin(async move { self.save_sync(scope_id, context) })
    }
}

impl std::fmt::Debug for SurrealDbKernelStore {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("SurrealDbKernelStore")
            .field("config", &self.config)
            .finish_non_exhaustive()
    }
}

impl std::fmt::Debug for SurrealDbContextStore {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("SurrealDbContextStore")
            .finish_non_exhaustive()
    }
}

impl SurrealDbKernelStore {
    pub async fn connect(config: AppConfig) -> StorageResult<Self> {
        let record_store = match &config.record_store {
            RecordStoreConfig::Surreal(record_store) => record_store.clone(),
            RecordStoreConfig::Memory => {
                return Err(StorageError::ConnectionFailed {
                    backend: "surrealdb".to_string(),
                    message: "record_store must be configured for surreal".to_string(),
                });
            }
        };

        let db = any::connect(record_store.endpoint.as_str())
            .await
            .map_err(|error| StorageError::ConnectionFailed {
                backend: "surrealdb".to_string(),
                message: error.to_string(),
            })?;

        if let (Some(username), Some(password)) = (
            record_store.username.as_deref(),
            record_store.password.as_deref(),
        ) {
            db.signin(Root {
                username: username.to_string(),
                password: password.to_string(),
            })
            .await
            .map_err(|error| StorageError::ConnectionFailed {
                backend: "surrealdb".to_string(),
                message: error.to_string(),
            })?;
        }

        db.use_ns(record_store.namespace.as_str())
            .use_db(record_store.database.as_str())
            .await
            .map_err(|error| StorageError::ConnectionFailed {
                backend: "surrealdb".to_string(),
                message: error.to_string(),
            })?;

        Ok(Self {
            db: Arc::new(db),
            write_lock: Arc::new(Mutex::new(())),
            runtime: None,
            config,
        })
    }

    pub fn connect_blocking(config: AppConfig) -> StorageResult<Self> {
        let runtime = Arc::new(
            RuntimeBuilder::new_multi_thread()
                .enable_all()
                .build()
                .map_err(|error| StorageError::Timeout {
                    operation: format!("build tokio runtime: {error}"),
                })?,
        );
        let mut store = runtime.block_on(Self::connect(config))?;
        store.runtime = Some(runtime);
        Ok(store)
    }

    pub fn read<R>(&self, f: impl FnOnce(&CrmKernel) -> R) -> StorageResult<R> {
        let kernel = self.load_kernel()?;
        Ok(f(&kernel))
    }

    pub fn write<R>(&self, f: impl FnOnce(&mut CrmKernel) -> KernelResult<R>) -> StorageResult<R> {
        self.write_with_events(f).map(|result| result.value)
    }

    pub fn write_with_events<R>(
        &self,
        f: impl FnOnce(&mut CrmKernel) -> KernelResult<R>,
    ) -> StorageResult<StoreWriteResult<R>> {
        let _guard = self
            .write_lock
            .lock()
            .map_err(|_| StorageError::LockPoisoned)?;

        let mut snapshot = self.load_kernel()?;
        let value = f(&mut snapshot)?;
        let events = snapshot.drain_events();
        self.save_kernel(&snapshot)?;
        Ok(StoreWriteResult { value, events })
    }

    fn load_kernel(&self) -> StorageResult<CrmKernel> {
        self.block_on(self.load_kernel_async())
    }

    fn save_kernel(&self, kernel: &CrmKernel) -> StorageResult<()> {
        self.block_on(self.save_kernel_async(kernel))
    }

    async fn load_kernel_async(&self) -> StorageResult<CrmKernel> {
        let document: Option<JsonValue> = match self
            .db
            .select((KERNEL_SNAPSHOT_TABLE, KERNEL_SNAPSHOT_ID))
            .await
        {
            Ok(document) => document,
            Err(error) => {
                let message = error.to_string();
                if message.contains(&format!("table '{KERNEL_SNAPSHOT_TABLE}' does not exist")) {
                    None
                } else {
                    return Err(StorageError::ConnectionFailed {
                        backend: "surrealdb".to_string(),
                        message,
                    });
                }
            }
        };
        match document {
            Some(document) => serde_json::from_value::<KernelSnapshotDocument>(document)
                .map(|value| value.kernel)
                .map_err(|error| StorageError::SerializationFailed {
                    message: error.to_string(),
                }),
            None => Ok(CrmKernel::default()),
        }
    }

    async fn save_kernel_async(&self, kernel: &CrmKernel) -> StorageResult<()> {
        let document = serde_json::to_value(KernelSnapshotDocument {
            kernel: kernel.clone(),
        })
        .map_err(|error| StorageError::SerializationFailed {
            message: error.to_string(),
        })?;

        let _: Option<JsonValue> = self
            .db
            .upsert((KERNEL_SNAPSHOT_TABLE, KERNEL_SNAPSHOT_ID))
            .content(document)
            .await
            .map_err(|error| StorageError::SerializationFailed {
                message: error.to_string(),
            })?;
        Ok(())
    }

    fn block_on<T>(&self, future: impl Future<Output = StorageResult<T>>) -> StorageResult<T> {
        if let Some(runtime) = &self.runtime {
            runtime.block_on(future)
        } else {
            block_on_storage(future)
        }
    }
}

impl KernelStore for SurrealDbKernelStore {
    fn read<R, F>(&self, f: F) -> StorageResult<R>
    where
        F: FnOnce(&CrmKernel) -> R,
    {
        SurrealDbKernelStore::read(self, f)
    }

    fn write_with_events<R, F>(&self, f: F) -> StorageResult<StoreWriteResult<R>>
    where
        F: FnOnce(&mut CrmKernel) -> KernelResult<R>,
    {
        SurrealDbKernelStore::write_with_events(self, f)
    }
}

impl KernelStore for AppKernelStore {
    fn read<R, F>(&self, f: F) -> StorageResult<R>
    where
        F: FnOnce(&CrmKernel) -> R,
    {
        match self {
            Self::Memory(store) => store.read(f),
            Self::Surreal(store) => store.read(f),
        }
    }

    fn write_with_events<R, F>(&self, f: F) -> StorageResult<StoreWriteResult<R>>
    where
        F: FnOnce(&mut CrmKernel) -> KernelResult<R>,
    {
        match self {
            Self::Memory(store) => store.write_with_events(f),
            Self::Surreal(store) => store.write_with_events(f),
        }
    }
}

impl AppContextStore {
    pub fn load_context_blocking(&self, scope_id: &str) -> StorageResult<Option<Context>> {
        match self {
            Self::Memory(store) => store
                .load_sync(scope_id)
                .map_err(map_store_to_storage_error),
            Self::Surreal(store) => store
                .load_sync(scope_id)
                .map_err(map_store_to_storage_error),
        }
    }

    pub fn save_context_blocking(&self, scope_id: &str, context: &Context) -> StorageResult<()> {
        match self {
            Self::Memory(store) => store
                .save_sync(scope_id, context)
                .map_err(map_store_to_storage_error),
            Self::Surreal(store) => store
                .save_sync(scope_id, context)
                .map_err(map_store_to_storage_error),
        }
    }
}

impl AppExperienceStore {
    pub fn append_blocking(&self, events: &[ExperienceEventEnvelope]) -> StorageResult<()> {
        let future = match self {
            Self::Memory(store) => store.append(events),
        };
        block_on_store(future).map_err(map_store_to_storage_error)
    }

    pub fn query_blocking(
        &self,
        query: &EventQuery,
    ) -> StorageResult<Vec<ExperienceEventEnvelope>> {
        let future = match self {
            Self::Memory(store) => store.query(query),
        };
        block_on_store(future).map_err(map_store_to_storage_error)
    }

    pub fn append_user_blocking(&self, event: UserExperienceEventEnvelope) -> StorageResult<()> {
        match self {
            Self::Memory(store) => store
                .append_user_sync(event)
                .map_err(map_store_to_storage_error),
        }
    }

    #[must_use]
    pub fn experience_handle(&self) -> Arc<converge_experience::InMemoryExperienceStore> {
        match self {
            Self::Memory(store) => store.handle(),
        }
    }
}

impl AppRuntimeStores {
    pub fn load_context(&self, scope_id: &str) -> StorageResult<Option<Context>> {
        self.context.load_context_blocking(scope_id)
    }

    pub fn save_context(&self, scope_id: &str, context: &Context) -> StorageResult<()> {
        self.context.save_context_blocking(scope_id, context)
    }

    pub fn append_experience_events(
        &self,
        events: &[ExperienceEventEnvelope],
    ) -> StorageResult<()> {
        if events.is_empty() {
            Ok(())
        } else {
            self.experience.append_blocking(events)
        }
    }

    pub fn append_user_event(&self, event: UserExperienceEventEnvelope) -> StorageResult<()> {
        self.experience.append_user_blocking(event)
    }

    #[must_use]
    pub fn experience_handle(&self) -> Arc<converge_experience::InMemoryExperienceStore> {
        self.experience.experience_handle()
    }
}

pub async fn open_kernel_store(config: AppConfig) -> StorageResult<AppKernelStore> {
    match config.record_store {
        RecordStoreConfig::Memory => Ok(AppKernelStore::Memory(InMemoryKernelStore::new(config))),
        RecordStoreConfig::Surreal(_) => SurrealDbKernelStore::connect(config)
            .await
            .map(AppKernelStore::Surreal),
    }
}

pub async fn open_runtime_stores(config: &AppConfig) -> StorageResult<AppRuntimeStores> {
    let context = match &config.context_store {
        ContextSnapshotStoreConfig::Memory => AppContextStore::Memory(InMemoryContextStore::new()),
        ContextSnapshotStoreConfig::Surreal(_) => {
            AppContextStore::Surreal(SurrealDbContextStore::connect(&config.context_store).await?)
        }
    };

    let experience = match &config.experience_store {
        ExperienceLedgerStoreConfig::Memory => {
            AppExperienceStore::Memory(InMemoryExperienceStoreAdapter::new())
        }
        ExperienceLedgerStoreConfig::Surreal(_) => {
            return Err(StorageError::RuntimeStore {
                message: "surreal-backed experience store is temporarily unavailable in this build"
                    .to_string(),
            });
        }
    };

    Ok(AppRuntimeStores {
        context,
        experience,
    })
}

fn block_on_storage<T>(future: impl Future<Output = StorageResult<T>>) -> StorageResult<T> {
    match Handle::try_current() {
        Ok(handle) => tokio::task::block_in_place(|| handle.block_on(future)),
        Err(_) => RuntimeBuilder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|error| StorageError::Timeout {
                operation: format!("build tokio runtime: {error}"),
            })?
            .block_on(future),
    }
}

fn block_on_store<T>(future: impl Future<Output = Result<T, StoreError>>) -> Result<T, StoreError> {
    match Handle::try_current() {
        Ok(handle) => tokio::task::block_in_place(|| handle.block_on(future)),
        Err(_) => RuntimeBuilder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|_error| StoreError::Timeout {
                elapsed: std::time::Duration::from_secs(0),
                deadline: std::time::Duration::from_secs(0),
            })?
            .block_on(future),
    }
}

fn map_storage_to_store_error(error: StorageError) -> StoreError {
    match error {
        StorageError::LockPoisoned => StoreError::Internal {
            message: "storage lock poisoned".to_string(),
        },
        StorageError::Kernel(error) => StoreError::InvariantViolation {
            message: error.to_string(),
        },
        StorageError::ConnectionFailed { message, .. } => StoreError::Unavailable { message },
        StorageError::SerializationFailed { message } => {
            StoreError::SerializationFailed { message }
        }
        StorageError::Timeout { .. } => StoreError::Timeout {
            elapsed: std::time::Duration::from_secs(0),
            deadline: std::time::Duration::from_secs(0),
        },
        StorageError::RuntimeStore { message } => StoreError::Internal { message },
    }
}

fn map_store_to_storage_error(error: StoreError) -> StorageError {
    match error {
        StoreError::Unavailable { message } => StorageError::RuntimeStore { message },
        StoreError::SerializationFailed { message } => StorageError::RuntimeStore { message },
        StoreError::Conflict { event_id } => StorageError::RuntimeStore {
            message: format!("conflicting event id: {event_id}"),
        },
        StoreError::InvalidQuery { message } => StorageError::RuntimeStore { message },
        StoreError::AuthFailed { message } => StorageError::RuntimeStore { message },
        StoreError::RateLimited { retry_after } => StorageError::RuntimeStore {
            message: format!("rate limited, retry after {:?}", retry_after),
        },
        StoreError::Timeout { elapsed, deadline } => StorageError::RuntimeStore {
            message: format!(
                "runtime store timed out after {:?} (deadline {:?})",
                elapsed, deadline
            ),
        },
        StoreError::NotFound { message } => StorageError::RuntimeStore { message },
        StoreError::InvariantViolation { message } => StorageError::RuntimeStore { message },
        StoreError::Internal { message } => StorageError::RuntimeStore { message },
    }
}

#[allow(deprecated)]
fn map_legacy_experience_error(error: converge_core::ExperienceStoreError) -> StoreError {
    match error {
        converge_core::ExperienceStoreError::StorageError { message } => {
            StoreError::Unavailable { message }
        }
        converge_core::ExperienceStoreError::InvalidQuery { message } => {
            StoreError::InvalidQuery { message }
        }
        converge_core::ExperienceStoreError::NotFound { message } => {
            StoreError::NotFound { message }
        }
    }
}

impl AppConfig {
    #[must_use]
    pub fn from_env() -> Self {
        let mut config = Self::default();
        let record_store = std::env::var("CRM_RECORD_STORE")
            .unwrap_or_else(|_| "memory".to_string())
            .to_ascii_lowercase();

        if record_store == "surreal" {
            let surreal =
                surreal_store_config_from_env("CRM_SURREAL_", "mem://", "crm_prio_ai", "local");
            config.record_store = RecordStoreConfig::Surreal(surreal.clone());
            config.context_store = ContextSnapshotStoreConfig::Surreal(surreal);
        }

        let context_store = std::env::var("CRM_CONTEXT_STORE")
            .ok()
            .map(|value| value.to_ascii_lowercase());
        if matches!(context_store.as_deref(), Some("memory")) {
            config.context_store = ContextSnapshotStoreConfig::Memory;
        } else if matches!(context_store.as_deref(), Some("surreal")) {
            config.context_store =
                ContextSnapshotStoreConfig::Surreal(surreal_store_config_from_env(
                    "CRM_CONTEXT_SURREAL_",
                    "mem://",
                    "crm_prio_ai",
                    "local",
                ));
        }

        let experience_store = std::env::var("CRM_EXPERIENCE_STORE")
            .unwrap_or_else(|_| "memory".to_string())
            .to_ascii_lowercase();
        if experience_store == "surreal" {
            config.experience_store =
                ExperienceLedgerStoreConfig::Surreal(surreal_store_config_from_env(
                    "CRM_EXPERIENCE_SURREAL_",
                    "ws://127.0.0.1:8000/rpc",
                    "crm_prio_ai",
                    "experience",
                ));
        }

        config
    }
}

fn surreal_store_config_from_env(
    prefix: &str,
    default_endpoint: &str,
    default_namespace: &str,
    default_database: &str,
) -> SurrealStoreConfig {
    SurrealStoreConfig {
        endpoint: std::env::var(format!("{prefix}ENDPOINT"))
            .unwrap_or_else(|_| default_endpoint.to_string()),
        namespace: std::env::var(format!("{prefix}NAMESPACE"))
            .unwrap_or_else(|_| default_namespace.to_string()),
        database: std::env::var(format!("{prefix}DATABASE"))
            .unwrap_or_else(|_| default_database.to_string()),
        username: std::env::var(format!("{prefix}USERNAME")).ok(),
        password: std::env::var(format!("{prefix}PASSWORD")).ok(),
    }
}

#[must_use]
pub fn runtime_modules_for_local_crm() -> Vec<RuntimeModuleConfig> {
    ConvergeFeatureConfig::default().runtime_modules
}

#[cfg(test)]
mod tests {
    use std::time::{SystemTime, UNIX_EPOCH};

    use application_kernel::{
        Actor, ActorKind, DomainEvent, OrganizationLifecycle, OrganizationUpsert,
    };

    use super::{
        AppConfig, InMemoryKernelStore, RecordStoreConfig, SurrealDbKernelStore, SurrealStoreConfig,
    };

    fn human() -> Actor {
        Actor {
            actor_id: "user-1".to_string(),
            display_name: "Kenneth".to_string(),
            kind: ActorKind::Human,
        }
    }

    fn surreal_test_config() -> AppConfig {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time should be after unix epoch")
            .as_nanos();
        AppConfig {
            record_store: RecordStoreConfig::Surreal(SurrealStoreConfig {
                endpoint: "mem://".to_string(),
                namespace: format!("crm_storage_test_{nonce}"),
                database: "local".to_string(),
                username: None,
                password: None,
            }),
            ..AppConfig::default()
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

    #[test]
    fn failed_write_does_not_commit_partial_state() {
        let store = InMemoryKernelStore::default_local();
        let actor = human();

        let result: super::StorageResult<super::StoreWriteResult<()>> =
            store.write_with_events(|kernel| {
                kernel.upsert_organization(
                    OrganizationUpsert {
                        organization_id: None,
                        name: "Should Roll Back".to_string(),
                        external_key: None,
                        website: None,
                        industry: None,
                        lifecycle: OrganizationLifecycle::Prospect,
                        owner_user_id: None,
                        tags: vec![],
                    },
                    actor.clone(),
                )?;
                Err(application_kernel::KernelError::Invariant(
                    "projection failure after first mutation".to_string(),
                ))
            });

        assert!(matches!(
            result,
            Err(super::StorageError::Kernel(
                application_kernel::KernelError::Invariant(_)
            ))
        ));
        assert_eq!(
            store
                .read(|kernel| kernel.organizations.len())
                .expect("read organizations after failed write"),
            0
        );
        assert_eq!(
            store
                .read(|kernel| kernel.pending_events.len())
                .expect("read pending events after failed write"),
            0
        );
    }

    #[test]
    fn surreal_store_supports_read_write_and_events() {
        let store =
            SurrealDbKernelStore::connect_blocking(surreal_test_config()).expect("connect store");
        let actor = human();

        let result = store
            .write_with_events(|kernel| {
                kernel.upsert_organization(
                    OrganizationUpsert {
                        organization_id: None,
                        name: "Surreal Aprio".to_string(),
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
        assert_eq!(
            store
                .read(|kernel| kernel.organizations.len())
                .expect("read organizations after surreal write"),
            1
        );
    }

    #[test]
    fn surreal_store_failed_write_does_not_commit_partial_state() {
        let store =
            SurrealDbKernelStore::connect_blocking(surreal_test_config()).expect("connect store");
        let actor = human();

        let result: super::StorageResult<super::StoreWriteResult<()>> =
            store.write_with_events(|kernel| {
                kernel.upsert_organization(
                    OrganizationUpsert {
                        organization_id: None,
                        name: "Should Roll Back".to_string(),
                        external_key: None,
                        website: None,
                        industry: None,
                        lifecycle: OrganizationLifecycle::Prospect,
                        owner_user_id: None,
                        tags: vec![],
                    },
                    actor.clone(),
                )?;
                Err(application_kernel::KernelError::Invariant(
                    "projection failure after first mutation".to_string(),
                ))
            });

        assert!(matches!(
            result,
            Err(super::StorageError::Kernel(
                application_kernel::KernelError::Invariant(_)
            ))
        ));
        assert_eq!(
            store
                .read(|kernel| kernel.organizations.len())
                .expect("read organizations after failed surreal write"),
            0
        );
    }
}

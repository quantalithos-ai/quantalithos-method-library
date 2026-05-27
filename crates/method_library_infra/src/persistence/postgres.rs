//! PostgreSQL persistence adapters.

use std::collections::HashMap;
use std::sync::Arc;

use async_trait::async_trait;
use method_library_application::ports::{
    AuditRecord, AuditRepository, CheckpointName, CheckpointStatus,
    ContentSummaryProjectionRepository, DefinitionSnapshotRepository,
    DefinitionTraceProjectionRepository, FailureReason, IdempotencyBeginResult,
    IdempotencyRepository, IdempotencyScope, InboundDeadLetter, InboundDeadLetterRepository,
    LifecycleHistoryEntry, LifecycleHistoryRepository, MethodContentReferenceRepository,
    MethodContentRepository, MethodContentVersionRecord, MethodContentVersionRepository,
    OutboxEvent, OutboxRepository, OutboxStatus, PageRequest, ProjectionCheckpointRecord,
    ProjectionCheckpointRepository, ResultRef, SupersedeLink, SupersedeLinkRepository,
    TransactionDriver, UnitOfWork, UnitOfWorkTx,
};
use method_library_contracts::{
    ContentSummaryView, DefinitionEventEnvelope, DefinitionSnapshot, DefinitionTraceView,
    ListMethodContentsQuery, RequestMeta,
};
use method_library_domain::content::{
    ContentId, ContentVersion, IdempotencyKey, LeaseDuration, MethodContent, MethodContentKind,
    OutboxEventId, PublishedContentRef, RequestHash, RequestId, Revision, SnapshotId, Timestamp,
    WorkerId,
};
use method_library_domain::{MethodLibraryError, MethodLibraryErrorCode};
use serde_json::Value as JsonValue;
use sqlx::pool::PoolConnection;
use sqlx::postgres::{PgPool, PgPoolOptions};
use sqlx::{Error as SqlxError, Row, types::Json};
use tokio::sync::Mutex as AsyncMutex;

const METHOD_CONTENTS_TABLE: &str = "method_contents";
const METHOD_CONTENT_REFERENCES_TABLE: &str = "method_content_references";
const METHOD_CONTENT_VERSIONS_TABLE: &str = "method_content_versions";
const SUPERSEDE_LINKS_TABLE: &str = "supersede_links";
const LIFECYCLE_HISTORY_TABLE: &str = "lifecycle_history_entries";
const AUDIT_RECORDS_TABLE: &str = "audit_records";
const OUTBOX_TABLE: &str = "outbox_events";
const IDEMPOTENCY_TABLE: &str = "idempotency_records";
const SNAPSHOT_TABLE: &str = "definition_snapshots";
const SUMMARY_TABLE: &str = "content_summary_projection";
const TRACE_TABLE: &str = "definition_trace_projection";
const CHECKPOINT_TABLE: &str = "projection_checkpoints";
const DEAD_LETTER_TABLE: &str = "inbound_dead_letters";

const UNIQUE_VERSION_CONSTRAINT: &str =
    "method_content_versions_content_family_id_version_text_key";
const UNIQUE_CONTENT_VERSION_CONSTRAINT: &str =
    "method_content_versions_content_id_version_text_key";
const UNIQUE_SUPERSEDE_CONSTRAINT: &str = "supersede_links_old_content_id_key";
const UNIQUE_IDEMPOTENCY_CONSTRAINT: &str = "idempotency_records_pkey";
const UNIQUE_SNAPSHOT_CONSTRAINT: &str =
    "definition_snapshots_content_id_version_text_fingerprint_value_key";
const UNIQUE_OUTBOX_IDEMPOTENCY_CONSTRAINT: &str = "outbox_events_idempotency_key_key";

type TransactionConnection = Arc<AsyncMutex<PoolConnection<sqlx::Postgres>>>;
type TransactionMap = HashMap<RequestId, TransactionConnection>;

/// Shared PostgreSQL-backed persistence state.
#[derive(Debug, Clone)]
pub struct PostgresPersistence {
    state: Arc<PostgresState>,
}

#[derive(Debug)]
struct PostgresState {
    pool: PgPool,
    transactions: AsyncMutex<TransactionMap>,
}

/// PostgreSQL unit-of-work adapter.
#[derive(Debug, Clone)]
pub struct PostgresUnitOfWork {
    state: Arc<PostgresState>,
}

/// PostgreSQL method-content repository adapter.
#[derive(Debug, Clone)]
pub struct PostgresMethodContentRepository {
    state: Arc<PostgresState>,
}

/// PostgreSQL reference repository adapter.
#[derive(Debug, Clone)]
pub struct PostgresMethodContentReferenceRepository {
    state: Arc<PostgresState>,
}

/// PostgreSQL version-history repository adapter.
#[derive(Debug, Clone)]
pub struct PostgresMethodContentVersionRepository {
    state: Arc<PostgresState>,
}

/// PostgreSQL supersede-link repository adapter.
#[derive(Debug, Clone)]
pub struct PostgresSupersedeLinkRepository {
    state: Arc<PostgresState>,
}

/// PostgreSQL lifecycle-history repository adapter.
#[derive(Debug, Clone)]
pub struct PostgresLifecycleHistoryRepository {
    state: Arc<PostgresState>,
}

/// PostgreSQL audit repository adapter.
#[derive(Debug, Clone)]
pub struct PostgresAuditRepository {
    state: Arc<PostgresState>,
}

/// PostgreSQL idempotency repository adapter.
#[derive(Debug, Clone)]
pub struct PostgresIdempotencyRepository {
    state: Arc<PostgresState>,
}

/// PostgreSQL outbox repository adapter.
#[derive(Debug, Clone)]
pub struct PostgresOutboxRepository {
    state: Arc<PostgresState>,
}

/// PostgreSQL snapshot repository adapter.
#[derive(Debug, Clone)]
pub struct PostgresDefinitionSnapshotRepository {
    state: Arc<PostgresState>,
}

/// PostgreSQL content-summary projection repository adapter.
#[derive(Debug, Clone)]
pub struct PostgresContentSummaryProjectionRepository {
    state: Arc<PostgresState>,
}

/// PostgreSQL definition-trace projection repository adapter.
#[derive(Debug, Clone)]
pub struct PostgresDefinitionTraceProjectionRepository {
    state: Arc<PostgresState>,
}

/// PostgreSQL checkpoint repository adapter.
#[derive(Debug, Clone)]
pub struct PostgresProjectionCheckpointRepository {
    state: Arc<PostgresState>,
}

/// PostgreSQL dead-letter repository adapter.
#[derive(Debug, Clone)]
pub struct PostgresInboundDeadLetterRepository {
    state: Arc<PostgresState>,
}

impl PostgresPersistence {
    /// Connects to PostgreSQL.
    pub async fn connect(database_url: &str) -> Result<Self, MethodLibraryError> {
        let pool = PgPoolOptions::new()
            .max_connections(5)
            .connect(database_url)
            .await
            .map_err(map_db_connect_error)?;

        Ok(Self {
            state: Arc::new(PostgresState {
                pool,
                transactions: AsyncMutex::new(HashMap::new()),
            }),
        })
    }

    /// Connects and runs migrations.
    pub async fn connect_and_migrate(database_url: &str) -> Result<Self, MethodLibraryError> {
        let persistence = Self::connect(database_url).await?;
        persistence.migrate().await?;
        Ok(persistence)
    }

    /// Runs the embedded migrations.
    pub async fn migrate(&self) -> Result<(), MethodLibraryError> {
        sqlx::migrate!("./migrations")
            .run(&self.state.pool)
            .await
            .map_err(map_migration_error)
    }

    /// Returns the unit-of-work adapter.
    #[must_use]
    pub fn unit_of_work(&self) -> PostgresUnitOfWork {
        PostgresUnitOfWork {
            state: self.state.clone(),
        }
    }

    /// Returns the method-content repository adapter.
    #[must_use]
    pub fn method_content_repository(&self) -> PostgresMethodContentRepository {
        PostgresMethodContentRepository {
            state: self.state.clone(),
        }
    }

    /// Returns the reference repository adapter.
    #[must_use]
    pub fn method_content_reference_repository(&self) -> PostgresMethodContentReferenceRepository {
        PostgresMethodContentReferenceRepository {
            state: self.state.clone(),
        }
    }

    /// Returns the version-history repository adapter.
    #[must_use]
    pub fn method_content_version_repository(&self) -> PostgresMethodContentVersionRepository {
        PostgresMethodContentVersionRepository {
            state: self.state.clone(),
        }
    }

    /// Returns the supersede-link repository adapter.
    #[must_use]
    pub fn supersede_link_repository(&self) -> PostgresSupersedeLinkRepository {
        PostgresSupersedeLinkRepository {
            state: self.state.clone(),
        }
    }

    /// Returns the lifecycle-history repository adapter.
    #[must_use]
    pub fn lifecycle_history_repository(&self) -> PostgresLifecycleHistoryRepository {
        PostgresLifecycleHistoryRepository {
            state: self.state.clone(),
        }
    }

    /// Returns the audit repository adapter.
    #[must_use]
    pub fn audit_repository(&self) -> PostgresAuditRepository {
        PostgresAuditRepository {
            state: self.state.clone(),
        }
    }

    /// Returns the idempotency repository adapter.
    #[must_use]
    pub fn idempotency_repository(&self) -> PostgresIdempotencyRepository {
        PostgresIdempotencyRepository {
            state: self.state.clone(),
        }
    }

    /// Returns the outbox repository adapter.
    #[must_use]
    pub fn outbox_repository(&self) -> PostgresOutboxRepository {
        PostgresOutboxRepository {
            state: self.state.clone(),
        }
    }

    /// Returns the snapshot repository adapter.
    #[must_use]
    pub fn snapshot_repository(&self) -> PostgresDefinitionSnapshotRepository {
        PostgresDefinitionSnapshotRepository {
            state: self.state.clone(),
        }
    }

    /// Returns the content-summary projection repository adapter.
    #[must_use]
    pub fn content_summary_projection_repository(
        &self,
    ) -> PostgresContentSummaryProjectionRepository {
        PostgresContentSummaryProjectionRepository {
            state: self.state.clone(),
        }
    }

    /// Returns the trace projection repository adapter.
    #[must_use]
    pub fn definition_trace_projection_repository(
        &self,
    ) -> PostgresDefinitionTraceProjectionRepository {
        PostgresDefinitionTraceProjectionRepository {
            state: self.state.clone(),
        }
    }

    /// Returns the projection checkpoint repository adapter.
    #[must_use]
    pub fn projection_checkpoint_repository(&self) -> PostgresProjectionCheckpointRepository {
        PostgresProjectionCheckpointRepository {
            state: self.state.clone(),
        }
    }

    /// Returns the dead-letter repository adapter.
    #[must_use]
    pub fn inbound_dead_letter_repository(&self) -> PostgresInboundDeadLetterRepository {
        PostgresInboundDeadLetterRepository {
            state: self.state.clone(),
        }
    }
}

impl PostgresState {
    async fn begin_transaction(
        self: &Arc<Self>,
        meta: RequestMeta,
    ) -> Result<UnitOfWorkTx, MethodLibraryError> {
        let request_id = meta.request_id.clone();
        let mut connection = self.pool.acquire().await.map_err(map_sqlx_error)?;
        sqlx::query("BEGIN")
            .execute(&mut *connection)
            .await
            .map_err(map_sqlx_error)?;

        let transaction: TransactionConnection = Arc::new(AsyncMutex::new(connection));
        let mut transactions = self.transactions.lock().await;
        if transactions.contains_key(&request_id) {
            drop(transactions);
            let mut connection = transaction.lock().await;
            let connection = &mut **connection;
            sqlx::query("ROLLBACK")
                .execute(&mut *connection)
                .await
                .map_err(map_sqlx_error)?;
            return Err(MethodLibraryError::validation(
                MethodLibraryErrorCode::TransactionCommitFailed,
                "a transaction for the same request id already exists",
            ));
        }

        transactions.insert(request_id.clone(), transaction);

        Ok(UnitOfWorkTx::new(
            meta,
            Arc::new(PostgresTransactionDriver {
                state: self.clone(),
            }),
        ))
    }

    async fn transaction_connection(
        &self,
        request_id: &str,
    ) -> Result<TransactionConnection, MethodLibraryError> {
        let transactions = self.transactions.lock().await;
        transactions.get(request_id).cloned().ok_or_else(|| {
            MethodLibraryError::retryable(
                MethodLibraryErrorCode::PersistenceUnavailable,
                "transaction connection is not registered",
            )
        })
    }

    async fn finish_transaction(
        self: &Arc<Self>,
        request_id: &str,
        commit: bool,
    ) -> Result<(), MethodLibraryError> {
        let transaction =
            { self.transactions.lock().await.remove(request_id) }.ok_or_else(|| {
                MethodLibraryError::retryable(
                    MethodLibraryErrorCode::PersistenceUnavailable,
                    "transaction connection is not registered",
                )
            })?;

        let mut connection = transaction.lock().await;
        let connection = &mut **connection;
        let statement = if commit { "COMMIT" } else { "ROLLBACK" };
        sqlx::query(statement)
            .execute(&mut *connection)
            .await
            .map_err(map_sqlx_error)?;

        Ok(())
    }
}

#[derive(Debug, Clone)]
struct PostgresTransactionDriver {
    state: Arc<PostgresState>,
}

#[async_trait]
impl TransactionDriver for PostgresTransactionDriver {
    async fn commit(&self, request_id: &RequestId) -> Result<(), MethodLibraryError> {
        self.state.finish_transaction(request_id, true).await
    }

    async fn rollback(&self, request_id: &RequestId) -> Result<(), MethodLibraryError> {
        self.state.finish_transaction(request_id, false).await
    }
}

#[async_trait]
impl UnitOfWork for PostgresUnitOfWork {
    async fn begin(&self, meta: RequestMeta) -> Result<UnitOfWorkTx, MethodLibraryError> {
        self.state.begin_transaction(meta).await
    }
}

#[async_trait]
impl MethodContentRepository for PostgresMethodContentRepository {
    async fn get(
        &self,
        content_id: ContentId,
    ) -> Result<Option<MethodContent>, MethodLibraryError> {
        let row = sqlx::query(&format!(
            "select content_json from {METHOD_CONTENTS_TABLE} where content_id = $1"
        ))
        .bind(&content_id)
        .fetch_optional(&self.state.pool)
        .await
        .map_err(map_sqlx_error)?;

        row.map(|row| -> Result<MethodContent, MethodLibraryError> {
            let Json(content): Json<MethodContent> =
                row.try_get("content_json").map_err(map_sqlx_error)?;
            MethodContent::rehydrate(content)
        })
        .transpose()
    }

    async fn find_published_by_kind(
        &self,
        kind: MethodContentKind,
    ) -> Result<Vec<MethodContent>, MethodLibraryError> {
        let rows = sqlx::query(&format!(
            "select content_json from {METHOD_CONTENTS_TABLE} where kind = $1 and lifecycle_state = 'published' order by content_id"
        ))
        .bind(kind.as_str())
        .fetch_all(&self.state.pool)
        .await
        .map_err(map_sqlx_error)?;

        rows.into_iter()
            .map(|row| -> Result<MethodContent, MethodLibraryError> {
                let Json(content): Json<MethodContent> =
                    row.try_get("content_json").map_err(map_sqlx_error)?;
                MethodContent::rehydrate(content)
            })
            .collect()
    }

    async fn get_for_update(
        &self,
        tx: &mut UnitOfWorkTx,
        content_id: ContentId,
    ) -> Result<Option<MethodContent>, MethodLibraryError> {
        let connection = self.state.transaction_connection(tx.request_id()).await?;
        let mut connection = connection.lock().await;
        let connection = &mut **connection;
        let row = sqlx::query(&format!(
            "select content_json from {METHOD_CONTENTS_TABLE} where content_id = $1 for update"
        ))
        .bind(&content_id)
        .fetch_optional(&mut *connection)
        .await
        .map_err(map_sqlx_error)?;

        row.map(|row| -> Result<MethodContent, MethodLibraryError> {
            let Json(content): Json<MethodContent> =
                row.try_get("content_json").map_err(map_sqlx_error)?;
            MethodContent::rehydrate(content)
        })
        .transpose()
    }

    async fn insert(
        &self,
        tx: &mut UnitOfWorkTx,
        content: MethodContent,
    ) -> Result<(), MethodLibraryError> {
        let connection = self.state.transaction_connection(tx.request_id()).await?;
        let mut connection = connection.lock().await;
        let connection = &mut **connection;
        sqlx::query(&format!(
            "insert into {METHOD_CONTENTS_TABLE} (content_id, content_family_id, kind, name, description, lifecycle_state, version_text, fingerprint_value, supersedes_content_id, superseded_by_content_id, created_by, created_at, updated_at, revision, content_json) values ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,$14,$15)"
        ))
        .bind(&content.content_id)
        .bind(&content.content_family_id)
        .bind(content.kind.as_str())
        .bind(&content.name)
        .bind(&content.description)
        .bind(content.lifecycle.state.as_str())
        .bind(content.version.as_ref().map(|version| version.raw.as_str()))
        .bind(content.fingerprint.as_ref().map(|fingerprint| fingerprint.value.as_str()))
        .bind(&content.supersedes_content_id)
        .bind(&content.superseded_by_content_id)
        .bind(&content.created_by)
        .bind(content.created_at)
        .bind(content.updated_at)
        .bind(content.revision)
        .bind(Json(&content))
        .execute(&mut *connection)
        .await
        .map(|_| ())
        .map_err(map_sqlx_error)
    }

    async fn save(
        &self,
        tx: &mut UnitOfWorkTx,
        content: MethodContent,
        expected_revision: Revision,
    ) -> Result<Revision, MethodLibraryError> {
        let connection = self.state.transaction_connection(tx.request_id()).await?;
        let mut connection = connection.lock().await;
        let connection = &mut **connection;
        let result = sqlx::query(&format!(
            "update {METHOD_CONTENTS_TABLE} set content_family_id = $2, kind = $3, name = $4, description = $5, lifecycle_state = $6, version_text = $7, fingerprint_value = $8, supersedes_content_id = $9, superseded_by_content_id = $10, created_by = $11, created_at = $12, updated_at = $13, revision = $14, content_json = $15 where content_id = $1 and revision = $16"
        ))
        .bind(&content.content_id)
        .bind(&content.content_family_id)
        .bind(content.kind.as_str())
        .bind(&content.name)
        .bind(&content.description)
        .bind(content.lifecycle.state.as_str())
        .bind(content.version.as_ref().map(|version| version.raw.as_str()))
        .bind(content.fingerprint.as_ref().map(|fingerprint| fingerprint.value.as_str()))
        .bind(&content.supersedes_content_id)
        .bind(&content.superseded_by_content_id)
        .bind(&content.created_by)
        .bind(content.created_at)
        .bind(content.updated_at)
        .bind(content.revision)
        .bind(Json(content.clone()))
        .bind(expected_revision)
        .execute(&mut *connection)
        .await
        .map_err(map_sqlx_error)?;

        if result.rows_affected() == 0 {
            let exists = sqlx::query_scalar::<_, bool>(&format!(
                "select exists(select 1 from {METHOD_CONTENTS_TABLE} where content_id = $1)"
            ))
            .bind(&content.content_id)
            .fetch_one(&mut *connection)
            .await
            .map_err(map_sqlx_error)?;

            return Err(if exists {
                MethodLibraryError::validation(
                    MethodLibraryErrorCode::RevisionConflict,
                    "expected revision does not match the stored revision",
                )
            } else {
                MethodLibraryError::validation(
                    MethodLibraryErrorCode::MethodContentNotFound,
                    "method content does not exist",
                )
            });
        }

        Ok(content.revision)
    }
}

#[async_trait]
impl MethodContentReferenceRepository for PostgresMethodContentReferenceRepository {
    async fn replace_refs(
        &self,
        tx: &mut UnitOfWorkTx,
        source_content_id: ContentId,
        refs: Vec<method_library_domain::content::ContentRef>,
    ) -> Result<(), MethodLibraryError> {
        let connection = self.state.transaction_connection(tx.request_id()).await?;
        let mut connection = connection.lock().await;
        let connection = &mut **connection;
        sqlx::query(&format!(
            "delete from {METHOD_CONTENT_REFERENCES_TABLE} where source_content_id = $1 and reference_kind = $2"
        ))
        .bind(&source_content_id)
        .bind("draft")
        .execute(&mut *connection)
        .await
        .map_err(map_sqlx_error)?;

        for reference in refs {
            sqlx::query(&format!(
                "insert into {METHOD_CONTENT_REFERENCES_TABLE} (source_content_id, reference_kind, target_content_id, target_kind, required_state, target_version_text, target_fingerprint_value, reference_json) values ($1,$2,$3,$4,$5,$6,$7,$8)"
            ))
            .bind(&source_content_id)
            .bind("draft")
            .bind(&reference.target_content_id)
            .bind(reference.target_kind.as_str())
            .bind(match reference.required_state {
                method_library_domain::content::ReferenceState::Published => "published",
                method_library_domain::content::ReferenceState::PublishedLike => "published_like",
            })
            .bind::<Option<&str>>(None)
            .bind::<Option<&str>>(None)
            .bind(Json(&reference))
            .execute(&mut *connection)
            .await
            .map_err(map_sqlx_error)?;
        }

        Ok(())
    }

    async fn replace_published_refs(
        &self,
        tx: &mut UnitOfWorkTx,
        source_content_id: ContentId,
        refs: Vec<PublishedContentRef>,
    ) -> Result<(), MethodLibraryError> {
        let connection = self.state.transaction_connection(tx.request_id()).await?;
        let mut connection = connection.lock().await;
        let connection = &mut **connection;
        sqlx::query(&format!(
            "delete from {METHOD_CONTENT_REFERENCES_TABLE} where source_content_id = $1 and reference_kind = $2"
        ))
        .bind(&source_content_id)
        .bind("published")
        .execute(&mut *connection)
        .await
        .map_err(map_sqlx_error)?;

        for reference in refs {
            sqlx::query(&format!(
                "insert into {METHOD_CONTENT_REFERENCES_TABLE} (source_content_id, reference_kind, target_content_id, target_kind, required_state, target_version_text, target_fingerprint_value, reference_json) values ($1,$2,$3,$4,$5,$6,$7,$8)"
            ))
            .bind(&source_content_id)
            .bind("published")
            .bind(&reference.content_id)
            .bind(reference.kind.as_str())
            .bind("published")
            .bind(Some(reference.version.raw.as_str()))
            .bind(Some(reference.fingerprint.value.as_str()))
            .bind(Json(&reference))
            .execute(&mut *connection)
            .await
            .map_err(map_sqlx_error)?;
        }

        Ok(())
    }

    async fn get_published_refs(
        &self,
        source_content_id: ContentId,
    ) -> Result<Vec<PublishedContentRef>, MethodLibraryError> {
        let rows = sqlx::query(&format!(
            "select reference_json from {METHOD_CONTENT_REFERENCES_TABLE} where source_content_id = $1 and reference_kind = $2 order by target_content_id"
        ))
        .bind(&source_content_id)
        .bind("published")
        .fetch_all(&self.state.pool)
        .await
        .map_err(map_sqlx_error)?;

        rows.into_iter()
            .map(|row| -> Result<PublishedContentRef, MethodLibraryError> {
                let Json(reference): Json<PublishedContentRef> =
                    row.try_get("reference_json").map_err(map_sqlx_error)?;
                Ok(reference)
            })
            .collect()
    }
}

#[async_trait]
impl MethodContentVersionRepository for PostgresMethodContentVersionRepository {
    async fn insert(
        &self,
        tx: &mut UnitOfWorkTx,
        record: MethodContentVersionRecord,
    ) -> Result<(), MethodLibraryError> {
        let connection = self.state.transaction_connection(tx.request_id()).await?;
        let mut connection = connection.lock().await;
        let connection = &mut **connection;
        let result = sqlx::query(&format!(
            "insert into {METHOD_CONTENT_VERSIONS_TABLE} (content_version_id, content_id, content_family_id, version_text, fingerprint_value, snapshot_id, published_at, version_json) values ($1,$2,$3,$4,$5,$6,$7,$8)"
        ))
        .bind(&record.content_version_id)
        .bind(&record.content_id)
        .bind(&record.content_family_id)
        .bind(&record.version.raw)
        .bind(&record.fingerprint.value)
        .bind(&record.snapshot_id)
        .bind(record.published_at)
        .bind(Json(&record))
        .execute(&mut *connection)
        .await;

        match result {
            Ok(_) => Ok(()),
            Err(error) => {
                if is_unique_violation(
                    &error,
                    &[UNIQUE_VERSION_CONSTRAINT, UNIQUE_CONTENT_VERSION_CONSTRAINT],
                ) {
                    Err(MethodLibraryError::validation(
                        MethodLibraryErrorCode::ContentVersionConflict,
                        "content version already exists",
                    ))
                } else {
                    Err(map_sqlx_error(error))
                }
            }
        }
    }

    async fn get(
        &self,
        content_id: ContentId,
        version: ContentVersion,
    ) -> Result<Option<MethodContentVersionRecord>, MethodLibraryError> {
        let row = sqlx::query(&format!(
            "select version_json from {METHOD_CONTENT_VERSIONS_TABLE} where content_id = $1 and version_text = $2"
        ))
        .bind(&content_id)
        .bind(&version.raw)
        .fetch_optional(&self.state.pool)
        .await
        .map_err(map_sqlx_error)?;

        row.map(
            |row| -> Result<MethodContentVersionRecord, MethodLibraryError> {
                let Json(record): Json<MethodContentVersionRecord> =
                    row.try_get("version_json").map_err(map_sqlx_error)?;
                Ok(record)
            },
        )
        .transpose()
    }
}

#[async_trait]
impl SupersedeLinkRepository for PostgresSupersedeLinkRepository {
    async fn insert(
        &self,
        tx: &mut UnitOfWorkTx,
        link: SupersedeLink,
    ) -> Result<(), MethodLibraryError> {
        let connection = self.state.transaction_connection(tx.request_id()).await?;
        let mut connection = connection.lock().await;
        let connection = &mut **connection;
        let result = sqlx::query(&format!(
            "insert into {SUPERSEDE_LINKS_TABLE} (supersede_link_id, old_content_id, new_content_id, content_family_id, reason, created_at) values ($1,$2,$3,$4,$5,$6)"
        ))
        .bind(&link.supersede_link_id)
        .bind(&link.old_content_id)
        .bind(&link.new_content_id)
        .bind(&link.content_family_id)
        .bind(&link.reason)
        .bind(link.created_at)
        .execute(&mut *connection)
        .await;

        match result {
            Ok(_) => Ok(()),
            Err(error) if is_unique_violation(&error, &[UNIQUE_SUPERSEDE_CONSTRAINT]) => {
                Err(MethodLibraryError::validation(
                    MethodLibraryErrorCode::SupersedeConflict,
                    "old content already has a supersede link",
                ))
            }
            Err(error) => Err(map_sqlx_error(error)),
        }
    }
}

#[async_trait]
impl LifecycleHistoryRepository for PostgresLifecycleHistoryRepository {
    async fn append(
        &self,
        tx: &mut UnitOfWorkTx,
        entry: LifecycleHistoryEntry,
    ) -> Result<(), MethodLibraryError> {
        let connection = self.state.transaction_connection(tx.request_id()).await?;
        let mut connection = connection.lock().await;
        let connection = &mut **connection;
        sqlx::query(&format!(
            "insert into {LIFECYCLE_HISTORY_TABLE} (history_entry_id, content_id, from_state, to_state, actor_id, reason, created_at, entry_json) values ($1,$2,$3,$4,$5,$6,$7,$8)"
        ))
        .bind(&entry.history_entry_id)
        .bind(&entry.content_id)
        .bind(entry.from_state.map(|state| state.as_str()))
        .bind(entry.to_state.as_str())
        .bind(&entry.actor_id)
        .bind(&entry.reason)
        .bind(entry.created_at)
        .bind(Json(&entry))
        .execute(&mut *connection)
        .await
        .map(|_| ())
        .map_err(map_sqlx_error)
    }
}

#[async_trait]
impl AuditRepository for PostgresAuditRepository {
    async fn append(
        &self,
        tx: &mut UnitOfWorkTx,
        record: AuditRecord,
    ) -> Result<(), MethodLibraryError> {
        let connection = self.state.transaction_connection(tx.request_id()).await?;
        let mut connection = connection.lock().await;
        let connection = &mut **connection;
        sqlx::query(&format!(
            "insert into {AUDIT_RECORDS_TABLE} (audit_id, request_id, trace_id, actor_context_json, target_ref_json, action, result, details_json, occurred_at, record_json) values ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10)"
        ))
        .bind(&record.audit_id)
        .bind(&record.request_id)
        .bind(&record.trace_id)
        .bind(Json(&record.actor_context))
        .bind(Json(&record.target_ref))
        .bind(&record.action)
        .bind(&record.result)
        .bind(Json(&record.details))
        .bind(record.occurred_at)
        .bind(Json(&record))
        .execute(&mut *connection)
        .await
        .map(|_| ())
        .map_err(map_sqlx_error)
    }
}

#[async_trait]
impl IdempotencyRepository for PostgresIdempotencyRepository {
    async fn try_begin(
        &self,
        tx: &mut UnitOfWorkTx,
        key: IdempotencyKey,
        scope: IdempotencyScope,
        request_hash: RequestHash,
        now: Timestamp,
    ) -> Result<IdempotencyBeginResult, MethodLibraryError> {
        let connection = self.state.transaction_connection(tx.request_id()).await?;
        let mut connection = connection.lock().await;
        let connection = &mut **connection;
        let existing = sqlx::query(&format!(
            "select scope, idempotency_key, request_hash, status, result_ref, failure_reason_json, updated_at from {IDEMPOTENCY_TABLE} where scope = $1 and idempotency_key = $2"
        ))
        .bind(&scope)
        .bind(&key)
        .fetch_optional(&mut *connection)
        .await
        .map_err(map_sqlx_error)?;

        if let Some(row) = existing {
            let stored_request_hash: String =
                row.try_get("request_hash").map_err(map_sqlx_error)?;
            if stored_request_hash != request_hash {
                return Err(MethodLibraryError::validation(
                    MethodLibraryErrorCode::IdempotencyConflict,
                    "idempotency key was reused with a different request hash",
                ));
            }

            let status: String = row.try_get("status").map_err(map_sqlx_error)?;
            return Ok(match status.as_str() {
                "processing" => IdempotencyBeginResult::Processing,
                "succeeded" => IdempotencyBeginResult::Succeeded(
                    row.try_get::<String, _>("result_ref")
                        .map_err(map_sqlx_error)?,
                ),
                "failed" => {
                    let failure_reason: JsonValue =
                        row.try_get("failure_reason_json").map_err(map_sqlx_error)?;
                    let reason = serde_json::from_value::<FailureReason>(failure_reason).map_err(
                        |error| {
                            MethodLibraryError::validation(
                                MethodLibraryErrorCode::IdempotencyStatusConflict,
                                error.to_string(),
                            )
                        },
                    )?;
                    IdempotencyBeginResult::Failed(reason)
                }
                _ => IdempotencyBeginResult::Processing,
            });
        }

        sqlx::query(&format!(
            "insert into {IDEMPOTENCY_TABLE} (scope, idempotency_key, request_hash, status, result_ref, failure_reason_json, updated_at) values ($1,$2,$3,$4,$5,$6,$7)"
        ))
        .bind(&scope)
        .bind(&key)
        .bind(&request_hash)
        .bind("processing")
        .bind::<Option<&str>>(None)
        .bind::<Option<JsonValue>>(None)
        .bind(now)
        .execute(&mut *connection)
        .await
        .map_err(map_sqlx_error)?;

        Ok(IdempotencyBeginResult::Started)
    }

    async fn mark_completed(
        &self,
        tx: &mut UnitOfWorkTx,
        key: IdempotencyKey,
        scope: IdempotencyScope,
        result_ref: ResultRef,
        now: Timestamp,
    ) -> Result<(), MethodLibraryError> {
        let connection = self.state.transaction_connection(tx.request_id()).await?;
        let mut connection = connection.lock().await;
        let connection = &mut **connection;
        let result = sqlx::query(&format!(
            "update {IDEMPOTENCY_TABLE} set status = 'succeeded', result_ref = $3, updated_at = $4 where scope = $1 and idempotency_key = $2 and status = 'processing'"
        ))
        .bind(&scope)
        .bind(&key)
        .bind(&result_ref)
        .bind(now)
        .execute(&mut *connection)
        .await
        .map_err(map_sqlx_error)?;

        if result.rows_affected() == 0 {
            return Err(MethodLibraryError::validation(
                MethodLibraryErrorCode::IdempotencyStatusConflict,
                "idempotency record is not in processing state",
            ));
        }

        Ok(())
    }

    async fn mark_failed(
        &self,
        tx: &mut UnitOfWorkTx,
        key: IdempotencyKey,
        scope: IdempotencyScope,
        reason: FailureReason,
        now: Timestamp,
    ) -> Result<(), MethodLibraryError> {
        let connection = self.state.transaction_connection(tx.request_id()).await?;
        let mut connection = connection.lock().await;
        let connection = &mut **connection;
        let result = sqlx::query(&format!(
            "update {IDEMPOTENCY_TABLE} set status = 'failed', failure_reason_json = $3, updated_at = $4 where scope = $1 and idempotency_key = $2 and status = 'processing'"
        ))
        .bind(&scope)
        .bind(&key)
        .bind(Json(&reason))
        .bind(now)
        .execute(&mut *connection)
        .await
        .map_err(map_sqlx_error)?;

        if result.rows_affected() == 0 {
            return Err(MethodLibraryError::validation(
                MethodLibraryErrorCode::IdempotencyStatusConflict,
                "idempotency record is not in processing state",
            ));
        }

        Ok(())
    }
}

#[async_trait]
impl OutboxRepository for PostgresOutboxRepository {
    async fn append(
        &self,
        tx: &mut UnitOfWorkTx,
        event: OutboxEvent,
    ) -> Result<(), MethodLibraryError> {
        let connection = self.state.transaction_connection(tx.request_id()).await?;
        let mut connection = connection.lock().await;
        let connection = &mut **connection;
        sqlx::query(&format!(
            "insert into {OUTBOX_TABLE} (outbox_event_id, aggregate_id, event_type, payload_json, payload_hash, status, retry_count, next_retry_at, worker_id, lease_until, published_at, idempotency_key) values ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12)"
        ))
        .bind(&event.event_id)
        .bind(&event.aggregate_id)
        .bind(event.envelope.event_type.as_str())
        .bind(Json(&event.envelope))
        .bind(&event.payload_hash)
        .bind("pending")
        .bind(0_i32)
        .bind::<Option<Timestamp>>(None)
        .bind::<Option<&str>>(None)
        .bind::<Option<Timestamp>>(None)
        .bind::<Option<Timestamp>>(None)
        .bind(&event.idempotency_key)
        .execute(&mut *connection)
        .await
        .map(|_| ())
        .map_err(map_sqlx_error)
    }

    async fn claim_pending(
        &self,
        limit: u32,
        worker_id: WorkerId,
        now: Timestamp,
        lease: LeaseDuration,
    ) -> Result<Vec<OutboxEvent>, MethodLibraryError> {
        let mut connection = self.state.pool.acquire().await.map_err(map_sqlx_error)?;
        let lease_until = now + lease;
        let rows = sqlx::query(&format!(
            "with claimable as (select outbox_event_id from {OUTBOX_TABLE} where status = 'pending' or (status = 'retryable_failed' and next_retry_at is not null and next_retry_at <= $2) or (status = 'publishing' and lease_until is not null and lease_until <= $2) order by outbox_event_id for update skip locked limit $1) update {OUTBOX_TABLE} o set status = 'publishing', worker_id = $3, lease_until = $4, next_retry_at = null from claimable c where o.outbox_event_id = c.outbox_event_id returning o.outbox_event_id, o.aggregate_id, o.payload_json, o.payload_hash, o.idempotency_key, o.retry_count"
        ))
        .bind(limit as i64)
        .bind(now)
        .bind(&worker_id)
        .bind(lease_until)
        .fetch_all(&mut *connection)
        .await
        .map_err(map_sqlx_error)?;

        rows.into_iter()
            .map(|row| -> Result<OutboxEvent, MethodLibraryError> {
                let Json(envelope): Json<DefinitionEventEnvelope> =
                    row.try_get("payload_json").map_err(map_sqlx_error)?;
                let retry_count: i32 = row.try_get("retry_count").map_err(map_sqlx_error)?;

                Ok(OutboxEvent {
                    event_id: row.try_get("outbox_event_id").map_err(map_sqlx_error)?,
                    aggregate_id: row.try_get("aggregate_id").map_err(map_sqlx_error)?,
                    envelope,
                    payload_hash: row.try_get("payload_hash").map_err(map_sqlx_error)?,
                    status: OutboxStatus::Publishing,
                    retry_count: retry_count as u32,
                    next_retry_at: None,
                    worker_id: Some(worker_id.clone()),
                    lease_until: Some(lease_until),
                    published_at: None,
                    idempotency_key: row.try_get("idempotency_key").map_err(map_sqlx_error)?,
                })
            })
            .collect()
    }

    async fn mark_published(
        &self,
        event_id: OutboxEventId,
        worker_id: WorkerId,
        now: Timestamp,
    ) -> Result<(), MethodLibraryError> {
        let rows = sqlx::query(&format!(
            "update {OUTBOX_TABLE} set status = 'published', published_at = $3, lease_until = null where outbox_event_id = $1 and worker_id = $2 and status = 'publishing'"
        ))
        .bind(&event_id)
        .bind(&worker_id)
        .bind(now)
        .execute(&self.state.pool)
        .await
        .map_err(map_sqlx_error)?;

        if rows.rows_affected() == 0 {
            return Err(MethodLibraryError::validation(
                MethodLibraryErrorCode::OutboxStatusConflict,
                "outbox event cannot be marked published by this worker",
            ));
        }

        Ok(())
    }

    async fn mark_retryable_failure(
        &self,
        event_id: OutboxEventId,
        worker_id: WorkerId,
        _reason: FailureReason,
        next_retry_at: Timestamp,
    ) -> Result<(), MethodLibraryError> {
        let rows = sqlx::query(&format!(
            "update {OUTBOX_TABLE} set status = 'retryable_failed', retry_count = retry_count + 1, next_retry_at = $3, lease_until = null where outbox_event_id = $1 and worker_id = $2 and status = 'publishing'"
        ))
        .bind(&event_id)
        .bind(&worker_id)
        .bind(next_retry_at)
        .execute(&self.state.pool)
        .await
        .map_err(map_sqlx_error)?;

        if rows.rows_affected() == 0 {
            return Err(MethodLibraryError::validation(
                MethodLibraryErrorCode::OutboxStatusConflict,
                "outbox event cannot be marked retryable failed by this worker",
            ));
        }

        Ok(())
    }

    async fn mark_dead_lettered(
        &self,
        event_id: OutboxEventId,
        worker_id: WorkerId,
        _reason: FailureReason,
        now: Timestamp,
    ) -> Result<(), MethodLibraryError> {
        let rows = sqlx::query(&format!(
            "update {OUTBOX_TABLE} set status = 'dead_lettered', published_at = coalesce(published_at, $3), lease_until = null where outbox_event_id = $1 and worker_id = $2 and status in ('publishing', 'retryable_failed')"
        ))
        .bind(&event_id)
        .bind(&worker_id)
        .bind(now)
        .execute(&self.state.pool)
        .await
        .map_err(map_sqlx_error)?;

        if rows.rows_affected() == 0 {
            return Err(MethodLibraryError::validation(
                MethodLibraryErrorCode::OutboxStatusConflict,
                "outbox event cannot be dead-lettered by this worker",
            ));
        }

        Ok(())
    }
}

#[async_trait]
impl DefinitionSnapshotRepository for PostgresDefinitionSnapshotRepository {
    async fn insert(
        &self,
        tx: &mut UnitOfWorkTx,
        snapshot: DefinitionSnapshot,
    ) -> Result<(), MethodLibraryError> {
        let connection = self.state.transaction_connection(tx.request_id()).await?;
        let mut connection = connection.lock().await;
        let connection = &mut **connection;
        let result = sqlx::query(&format!(
            "insert into {SNAPSHOT_TABLE} (snapshot_id, content_id, version_text, fingerprint_value, schema_version, blob_ref, created_at, content_ref_json, references_json, snapshot_json) values ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10)"
        ))
        .bind(&snapshot.snapshot_id)
        .bind(&snapshot.content_id)
        .bind(&snapshot.version.raw)
        .bind(&snapshot.fingerprint.value)
        .bind(&snapshot.schema_version)
        .bind(&snapshot.blob_ref)
        .bind(snapshot.created_at)
        .bind(Json(&snapshot.content_ref))
        .bind(Json(&snapshot.references))
        .bind(Json(&snapshot))
        .execute(&mut *connection)
        .await;

        match result {
            Ok(_) => Ok(()),
            Err(error) if is_unique_violation(&error, &[UNIQUE_SNAPSHOT_CONSTRAINT]) => {
                Err(MethodLibraryError::validation(
                    MethodLibraryErrorCode::SnapshotBuildFailed,
                    "definition snapshot already exists",
                ))
            }
            Err(error) => Err(map_sqlx_error(error)),
        }
    }

    async fn get(
        &self,
        snapshot_id: SnapshotId,
    ) -> Result<Option<DefinitionSnapshot>, MethodLibraryError> {
        let row = sqlx::query(&format!(
            "select snapshot_json from {SNAPSHOT_TABLE} where snapshot_id = $1"
        ))
        .bind(&snapshot_id)
        .fetch_optional(&self.state.pool)
        .await
        .map_err(map_sqlx_error)?;

        row.map(|row| -> Result<DefinitionSnapshot, MethodLibraryError> {
            let Json(snapshot): Json<DefinitionSnapshot> =
                row.try_get("snapshot_json").map_err(map_sqlx_error)?;
            Ok(snapshot)
        })
        .transpose()
    }

    async fn get_by_content_version(
        &self,
        content_id: ContentId,
        version: ContentVersion,
    ) -> Result<Option<DefinitionSnapshot>, MethodLibraryError> {
        let row = sqlx::query(&format!(
            "select snapshot_json from {SNAPSHOT_TABLE} where content_id = $1 and version_text = $2"
        ))
        .bind(&content_id)
        .bind(&version.raw)
        .fetch_optional(&self.state.pool)
        .await
        .map_err(map_sqlx_error)?;

        row.map(|row| -> Result<DefinitionSnapshot, MethodLibraryError> {
            let Json(snapshot): Json<DefinitionSnapshot> =
                row.try_get("snapshot_json").map_err(map_sqlx_error)?;
            Ok(snapshot)
        })
        .transpose()
    }
}

#[async_trait]
impl ContentSummaryProjectionRepository for PostgresContentSummaryProjectionRepository {
    async fn upsert(&self, view: ContentSummaryView) -> Result<(), MethodLibraryError> {
        sqlx::query(&format!(
            "insert into {SUMMARY_TABLE} (content_id, kind, name, lifecycle_state, version_text, fingerprint_value, updated_at, view_json) values ($1,$2,$3,$4,$5,$6,$7,$8) on conflict (content_id) do update set kind = excluded.kind, name = excluded.name, lifecycle_state = excluded.lifecycle_state, version_text = excluded.version_text, fingerprint_value = excluded.fingerprint_value, updated_at = excluded.updated_at, view_json = excluded.view_json"
        ))
        .bind(&view.content_id)
        .bind(view.kind.as_str())
        .bind(&view.name)
        .bind(view.lifecycle_state.as_str())
        .bind(view.version.as_ref().map(|version| version.raw.as_str()))
        .bind::<Option<&str>>(None)
        .bind(view.updated_at)
        .bind(Json(&view))
        .execute(&self.state.pool)
        .await
        .map(|_| ())
        .map_err(map_sqlx_error)
    }

    async fn list(
        &self,
        query: &ListMethodContentsQuery,
        page: &PageRequest,
    ) -> Result<Vec<ContentSummaryView>, MethodLibraryError> {
        let mut sql = format!("select view_json from {SUMMARY_TABLE} where 1 = 1");
        let mut conditions: Vec<String> = Vec::new();
        let mut bind_index = 1;
        if query.read_mode == method_library_contracts::ReadMode::Published {
            conditions.push("lifecycle_state in ('published', 'deprecated')".to_string());
        }
        if query.kind.is_some() {
            conditions.push(format!("kind = ${bind_index}"));
            bind_index += 1;
        }
        if query.lifecycle_state.is_some() {
            conditions.push(format!("lifecycle_state = ${bind_index}"));
            bind_index += 1;
        }
        if page.cursor.is_some() {
            conditions.push(format!("content_id > ${bind_index}"));
        }
        if !conditions.is_empty() {
            sql.push_str(" and ");
            sql.push_str(&conditions.join(" and "));
        }
        sql.push_str(" order by content_id limit ");
        sql.push_str(&(page.limit as i64).to_string());

        let mut query_builder = sqlx::query(&sql);
        if let Some(kind) = query.kind {
            query_builder = query_builder.bind(kind.as_str());
        }
        if let Some(state) = query.lifecycle_state {
            query_builder = query_builder.bind(state.as_str());
        }
        if let Some(cursor) = &page.cursor {
            query_builder = query_builder.bind(cursor);
        }

        let rows = query_builder
            .fetch_all(&self.state.pool)
            .await
            .map_err(map_sqlx_error)?;

        rows.into_iter()
            .map(|row| -> Result<ContentSummaryView, MethodLibraryError> {
                let Json(view): Json<ContentSummaryView> =
                    row.try_get("view_json").map_err(map_sqlx_error)?;
                Ok(view)
            })
            .collect()
    }
}

#[async_trait]
impl DefinitionTraceProjectionRepository for PostgresDefinitionTraceProjectionRepository {
    async fn upsert(&self, view: DefinitionTraceView) -> Result<(), MethodLibraryError> {
        sqlx::query(&format!(
            "insert into {TRACE_TABLE} (content_id, trace_json, updated_at) values ($1,$2,$3) on conflict (content_id) do update set trace_json = excluded.trace_json, updated_at = excluded.updated_at"
        ))
        .bind(&view.content_id)
        .bind(Json(&view))
        .bind(now_timestamp())
        .execute(&self.state.pool)
        .await
        .map(|_| ())
        .map_err(map_sqlx_error)
    }

    async fn get(
        &self,
        content_id: ContentId,
    ) -> Result<Option<DefinitionTraceView>, MethodLibraryError> {
        let row = sqlx::query(&format!(
            "select trace_json from {TRACE_TABLE} where content_id = $1"
        ))
        .bind(&content_id)
        .fetch_optional(&self.state.pool)
        .await
        .map_err(map_sqlx_error)?;

        row.map(|row| -> Result<DefinitionTraceView, MethodLibraryError> {
            let Json(view): Json<DefinitionTraceView> =
                row.try_get("trace_json").map_err(map_sqlx_error)?;
            Ok(view)
        })
        .transpose()
    }
}

#[async_trait]
impl ProjectionCheckpointRepository for PostgresProjectionCheckpointRepository {
    async fn advance_if_current(
        &self,
        name: CheckpointName,
        expected_cursor: Option<OutboxEventId>,
        next_cursor: OutboxEventId,
        now: Timestamp,
    ) -> Result<(), MethodLibraryError> {
        let rows = sqlx::query(&format!(
            "update {CHECKPOINT_TABLE} set last_processed_event_id = $3, status = 'active', updated_at = $4 where checkpoint_name = $1 and last_processed_event_id is not distinct from $2"
        ))
        .bind(&name)
        .bind(&expected_cursor)
        .bind(&next_cursor)
        .bind(now)
        .execute(&self.state.pool)
        .await
        .map_err(map_sqlx_error)?;

        if rows.rows_affected() > 0 {
            return Ok(());
        }

        if expected_cursor.is_none() {
            let inserted = sqlx::query(&format!(
                "insert into {CHECKPOINT_TABLE} (checkpoint_name, last_processed_event_id, status, updated_at) values ($1,$2,'active',$3) on conflict do nothing"
            ))
            .bind(&name)
            .bind(&next_cursor)
            .bind(now)
            .execute(&self.state.pool)
            .await
            .map_err(map_sqlx_error)?;
            if inserted.rows_affected() > 0 {
                return Ok(());
            }
        }

        Err(MethodLibraryError::validation(
            MethodLibraryErrorCode::CheckpointConflict,
            "projection checkpoint compare-and-swap failed",
        ))
    }

    async fn get(
        &self,
        name: CheckpointName,
    ) -> Result<Option<ProjectionCheckpointRecord>, MethodLibraryError> {
        let row = sqlx::query(&format!(
            "select checkpoint_name, last_processed_event_id, status, updated_at from {CHECKPOINT_TABLE} where checkpoint_name = $1"
        ))
        .bind(&name)
        .fetch_optional(&self.state.pool)
        .await
        .map_err(map_sqlx_error)?;

        row.map(
            |row| -> Result<ProjectionCheckpointRecord, MethodLibraryError> {
                Ok(ProjectionCheckpointRecord {
                    checkpoint_name: row.try_get("checkpoint_name").map_err(map_sqlx_error)?,
                    last_processed_event_id: row
                        .try_get("last_processed_event_id")
                        .map_err(map_sqlx_error)?,
                    status: match row
                        .try_get::<String, _>("status")
                        .map_err(map_sqlx_error)?
                        .as_str()
                    {
                        "active" => CheckpointStatus::Active,
                        "rebuilding" => CheckpointStatus::Rebuilding,
                        _ => CheckpointStatus::Failed,
                    },
                    updated_at: row.try_get("updated_at").map_err(map_sqlx_error)?,
                })
            },
        )
        .transpose()
    }
}

#[async_trait]
impl InboundDeadLetterRepository for PostgresInboundDeadLetterRepository {
    async fn append(&self, record: InboundDeadLetter) -> Result<(), MethodLibraryError> {
        sqlx::query(&format!(
            "insert into {DEAD_LETTER_TABLE} (dead_letter_id, source_module, event_type, payload_json, failure_reason_json, created_at) values ($1,$2,$3,$4,$5,$6)"
        ))
        .bind(&record.dead_letter_id)
        .bind(&record.source_module)
        .bind(&record.event_type)
        .bind(Json(&record.payload))
        .bind(Json(&record.failure_reason))
        .bind(record.created_at)
        .execute(&self.state.pool)
        .await
        .map(|_| ())
        .map_err(map_sqlx_error)
    }
}

fn map_db_connect_error(error: sqlx::Error) -> MethodLibraryError {
    MethodLibraryError::retryable(
        MethodLibraryErrorCode::PersistenceUnavailable,
        format!("failed to connect to PostgreSQL: {error}"),
    )
}

fn map_migration_error(error: sqlx::migrate::MigrateError) -> MethodLibraryError {
    MethodLibraryError::retryable(
        MethodLibraryErrorCode::PersistenceUnavailable,
        format!("failed to apply PostgreSQL migrations: {error}"),
    )
}

fn map_sqlx_error(error: SqlxError) -> MethodLibraryError {
    match error {
        SqlxError::RowNotFound => MethodLibraryError::validation(
            MethodLibraryErrorCode::MethodContentNotFound,
            "row not found",
        ),
        SqlxError::Database(db_error) => {
            let code = db_error.code().map(|code| code.to_string());
            let constraint = db_error.constraint().map(|value| value.to_string());
            if matches!(code.as_deref(), Some("23505")) {
                if matches!(
                    constraint.as_deref(),
                    Some(UNIQUE_VERSION_CONSTRAINT) | Some(UNIQUE_CONTENT_VERSION_CONSTRAINT)
                ) {
                    return MethodLibraryError::validation(
                        MethodLibraryErrorCode::ContentVersionConflict,
                        "unique version constraint violated",
                    );
                }
                if matches!(constraint.as_deref(), Some(UNIQUE_SUPERSEDE_CONSTRAINT)) {
                    return MethodLibraryError::validation(
                        MethodLibraryErrorCode::SupersedeConflict,
                        "unique supersede constraint violated",
                    );
                }
                if matches!(constraint.as_deref(), Some(UNIQUE_SNAPSHOT_CONSTRAINT)) {
                    return MethodLibraryError::validation(
                        MethodLibraryErrorCode::SnapshotBuildFailed,
                        "unique snapshot constraint violated",
                    );
                }
                if matches!(constraint.as_deref(), Some(UNIQUE_IDEMPOTENCY_CONSTRAINT)) {
                    return MethodLibraryError::validation(
                        MethodLibraryErrorCode::IdempotencyConflict,
                        "unique idempotency constraint violated",
                    );
                }
                if matches!(
                    constraint.as_deref(),
                    Some(UNIQUE_OUTBOX_IDEMPOTENCY_CONSTRAINT)
                ) {
                    return MethodLibraryError::validation(
                        MethodLibraryErrorCode::IdempotencyConflict,
                        "unique outbox idempotency constraint violated",
                    );
                }
            }

            MethodLibraryError::retryable(
                MethodLibraryErrorCode::PersistenceUnavailable,
                format!("database error: {db_error}"),
            )
        }
        other => MethodLibraryError::retryable(
            MethodLibraryErrorCode::PersistenceUnavailable,
            format!("sqlx error: {other}"),
        ),
    }
}

fn is_unique_violation(error: &SqlxError, constraints: &[&str]) -> bool {
    match error {
        SqlxError::Database(db_error) => {
            db_error.code().is_some_and(|code| code == "23505")
                && db_error
                    .constraint()
                    .is_some_and(|value| constraints.contains(&value))
        }
        _ => false,
    }
}

fn now_timestamp() -> Timestamp {
    time::OffsetDateTime::now_utc()
}

/// Test utilities for PostgreSQL persistence.
pub struct PostgresTestDatabase;

impl PostgresTestDatabase {
    /// Returns the test database URL.
    #[must_use]
    pub fn database_url() -> String {
        std::env::var("METHOD_LIBRARY_TEST_DATABASE_URL").unwrap_or_else(|_| {
            "postgres://postgres:postgres@127.0.0.1:5432/quantalithos_method_library_test"
                .to_string()
        })
    }

    /// Creates the test database if needed.
    pub async fn ensure_database() -> Result<(), MethodLibraryError> {
        let admin_url = std::env::var("METHOD_LIBRARY_TEST_ADMIN_DATABASE_URL")
            .unwrap_or_else(|_| "postgres://postgres:postgres@127.0.0.1:5432/postgres".to_string());
        let pool = PgPoolOptions::new()
            .max_connections(1)
            .connect(&admin_url)
            .await
            .map_err(map_db_connect_error)?;

        let db_name = "quantalithos_method_library_test";
        let exists: Option<String> =
            sqlx::query_scalar("select datname from pg_database where datname = $1")
                .bind(db_name)
                .fetch_optional(&pool)
                .await
                .map_err(map_sqlx_error)?;

        if exists.is_none() {
            sqlx::query(&format!("create database {db_name}"))
                .execute(&pool)
                .await
                .map_err(map_sqlx_error)?;
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex, OnceLock};

    use time::macros::datetime;

    use super::*;
    use method_library_application::ports::SupersedeLink;
    use method_library_application::ports::fakes::{
        DeterministicClock, DeterministicFingerprintHasher, DeterministicIdGenerator,
        InMemoryObjectStorage, StaticGovernancePort,
    };
    use method_library_application::{MethodContentCommandService, UnitOfWork};
    use method_library_contracts::{
        ActorContext, ContentSummaryView, EventTraceContext, ListMethodContentsQuery,
        PublishMethodContentCommand, ReadMode, RequestMeta, SupersedeMethodContentCommand,
    };
    use method_library_domain::content::{
        ActorKind, ApprovedGateRef, CanonicalFingerprint, ContentVersion, FingerprintAlgorithm,
        LifecycleState, MethodContent, MethodContentKind, PublishedContentRef,
    };
    use method_library_domain::definitions::{
        EvidenceKind, EvidenceRule, MethodContentPayload, Qualification, QualificationLevel,
        QualificationLevelModel,
    };

    static TEST_LOCK: OnceLock<Mutex<()>> = OnceLock::new();

    fn test_guard() -> std::sync::MutexGuard<'static, ()> {
        TEST_LOCK
            .get_or_init(|| Mutex::new(()))
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    async fn test_db() -> PostgresPersistence {
        PostgresTestDatabase::ensure_database()
            .await
            .expect("test database should exist");
        let persistence =
            PostgresPersistence::connect_and_migrate(&PostgresTestDatabase::database_url())
                .await
                .expect("test database should connect");
        sqlx::query(&format!("truncate table {METHOD_CONTENTS_TABLE}, {METHOD_CONTENT_REFERENCES_TABLE}, {METHOD_CONTENT_VERSIONS_TABLE}, {SUPERSEDE_LINKS_TABLE}, {LIFECYCLE_HISTORY_TABLE}, {AUDIT_RECORDS_TABLE}, {OUTBOX_TABLE}, {IDEMPOTENCY_TABLE}, {SNAPSHOT_TABLE}, {SUMMARY_TABLE}, {TRACE_TABLE}, {CHECKPOINT_TABLE}, {DEAD_LETTER_TABLE} restart identity cascade"))
            .execute(&persistence.state.pool)
            .await
            .expect("tables should truncate");
        persistence
    }

    fn sample_meta() -> RequestMeta {
        RequestMeta {
            request_id: "req-1".to_string(),
            trace_id: "trace-1".to_string(),
            idempotency_key: Some("idem-1".to_string()),
            request_hash: "hash-1".to_string(),
            received_at: datetime!(2026-05-26 08:00:00 UTC),
        }
    }

    fn sample_content() -> MethodContent {
        MethodContent::create_draft(
            "content-1".to_string(),
            "family-1".to_string(),
            MethodContentKind::Qualification,
            "Quality".to_string(),
            None,
            MethodContentPayload::Qualification(Qualification {
                qualification_key: "quality-1".to_string(),
                name: "Quality".to_string(),
                description: None,
                level_model: QualificationLevelModel {
                    levels: vec![QualificationLevel {
                        level_key: "basic".to_string(),
                        name: "Basic".to_string(),
                        order: 1,
                        description: None,
                    }],
                    default_level_key: Some("basic".to_string()),
                },
                evidence_rules: vec![EvidenceRule {
                    evidence_kind: EvidenceKind::Document,
                    required: true,
                    description: "Proof".to_string(),
                }],
            }),
            "actor-1".to_string(),
            datetime!(2026-05-26 08:00:00 UTC),
        )
        .expect("content should be valid")
    }

    fn sample_version_record(
        content_version_id: &str,
        content_id: &str,
        content_family_id: &str,
        version: &str,
    ) -> MethodContentVersionRecord {
        MethodContentVersionRecord {
            content_version_id: content_version_id.to_string(),
            content_id: content_id.to_string(),
            content_family_id: content_family_id.to_string(),
            version: ContentVersion::new(version).expect("version should be valid"),
            fingerprint: CanonicalFingerprint::new(
                FingerprintAlgorithm::Sha256,
                format!("{content_id}-{version}"),
                "1.0",
            )
            .expect("fingerprint should be valid"),
            snapshot_id: format!("snap-{content_version_id}"),
            published_at: datetime!(2026-05-26 08:00:00 UTC),
        }
    }

    fn sample_actor() -> ActorContext {
        ActorContext {
            actor_id: "actor-1".to_string(),
            actor_kind: ActorKind::Human,
            actor_ref: method_library_contracts::ActorRef {
                actor_id: "actor-1".to_string(),
                actor_kind: ActorKind::Human,
            },
        }
    }

    fn sample_summary_view(
        content_id: &str,
        lifecycle_state: LifecycleState,
    ) -> ContentSummaryView {
        ContentSummaryView {
            content_id: content_id.to_string(),
            kind: MethodContentKind::Qualification,
            name: format!("Summary {content_id}"),
            lifecycle_state,
            version: matches!(
                lifecycle_state,
                LifecycleState::Published | LifecycleState::Deprecated
            )
            .then(|| ContentVersion::new("1.0.0").expect("version should be valid")),
            updated_at: datetime!(2026-05-26 08:00:00 UTC),
        }
    }

    fn sample_meta_with(idempotency_key: &str, request_hash: &str) -> RequestMeta {
        RequestMeta {
            request_id: "req-1".to_string(),
            trace_id: "trace-1".to_string(),
            idempotency_key: Some(idempotency_key.to_string()),
            request_hash: request_hash.to_string(),
            received_at: datetime!(2026-05-26 08:00:00 UTC),
        }
    }

    fn command_service(persistence: &PostgresPersistence) -> MethodContentCommandService {
        MethodContentCommandService::new(
            Arc::new(persistence.unit_of_work()),
            Arc::new(persistence.method_content_repository()),
            Arc::new(persistence.method_content_reference_repository()),
            Arc::new(persistence.method_content_version_repository()),
            Arc::new(persistence.snapshot_repository()),
            Arc::new(persistence.supersede_link_repository()),
            Arc::new(persistence.outbox_repository()),
            Arc::new(persistence.lifecycle_history_repository()),
            Arc::new(persistence.audit_repository()),
            Arc::new(persistence.idempotency_repository()),
            Arc::new(StaticGovernancePort::new(
                true,
                datetime!(2026-05-26 08:00:00 UTC),
            )),
            Arc::new(InMemoryObjectStorage::default()),
            Arc::new(DeterministicFingerprintHasher::default()),
            Arc::new(DeterministicClock::new(datetime!(2026-05-26 08:00:00 UTC))),
            Arc::new(DeterministicIdGenerator::default()),
        )
    }

    fn sample_in_review_content(content_id: &str, family_id: &str) -> MethodContent {
        let mut content = MethodContent::create_draft(
            content_id.to_string(),
            family_id.to_string(),
            MethodContentKind::Qualification,
            format!("Quality {content_id}"),
            None,
            MethodContentPayload::Qualification(Qualification {
                qualification_key: format!("quality-{content_id}"),
                name: format!("Quality {content_id}"),
                description: None,
                level_model: QualificationLevelModel {
                    levels: vec![QualificationLevel {
                        level_key: "basic".to_string(),
                        name: "Basic".to_string(),
                        order: 1,
                        description: None,
                    }],
                    default_level_key: Some("basic".to_string()),
                },
                evidence_rules: vec![EvidenceRule {
                    evidence_kind: EvidenceKind::Document,
                    required: true,
                    description: "Proof".to_string(),
                }],
            }),
            "actor-1".to_string(),
            datetime!(2026-05-26 08:00:00 UTC),
        )
        .expect("fixture content should build");
        content
            .submit_for_review("actor-1".to_string(), datetime!(2026-05-26 08:05:00 UTC))
            .expect("fixture should enter review");
        content
    }

    fn sample_published_content(content_id: &str, family_id: &str) -> MethodContent {
        let mut content = sample_in_review_content(content_id, family_id);
        content
            .publish(
                ApprovedGateRef {
                    gate_id: "gate-1".to_string(),
                    gate_decision_id: "decision-1".to_string(),
                    approved_at: datetime!(2026-05-26 08:10:00 UTC),
                },
                ContentVersion::new("1.0.0").expect("version should be valid"),
                CanonicalFingerprint::new(FingerprintAlgorithm::Sha256, "abc123", "1.0")
                    .expect("fingerprint should be valid"),
                "actor-1".to_string(),
                datetime!(2026-05-26 08:10:00 UTC),
            )
            .expect("fixture should publish");
        content
    }

    async fn insert_content(
        persistence: &PostgresPersistence,
        content: MethodContent,
        idempotency_key: &str,
    ) {
        let repository = persistence.method_content_repository();
        let mut tx = persistence
            .unit_of_work()
            .begin(sample_meta_with(
                idempotency_key,
                &format!("hash-{idempotency_key}"),
            ))
            .await
            .expect("transaction should begin");
        repository
            .insert(&mut tx, content)
            .await
            .expect("content should insert");
        tx.commit().await.expect("commit should succeed");
    }

    fn sample_publish_command(
        content_id: &str,
        expected_revision: i64,
    ) -> PublishMethodContentCommand {
        PublishMethodContentCommand {
            content_id: content_id.to_string(),
            expected_revision,
            version: ContentVersion::new("1.0.0").expect("version should be valid"),
            approved_gate_ref: ApprovedGateRef {
                gate_id: "gate-1".to_string(),
                gate_decision_id: "decision-1".to_string(),
                approved_at: datetime!(2026-05-26 08:10:00 UTC),
            },
            publish_reason: "Initial release".to_string(),
        }
    }

    fn sample_supersede_command(
        old_content_id: &str,
        old_expected_revision: i64,
        new_content_id: &str,
        new_expected_revision: i64,
    ) -> SupersedeMethodContentCommand {
        SupersedeMethodContentCommand {
            old_content_id: old_content_id.to_string(),
            old_expected_revision,
            new_content_id: new_content_id.to_string(),
            new_expected_revision,
            new_version: ContentVersion::new("2.0.0").expect("version should be valid"),
            approved_gate_ref: ApprovedGateRef {
                gate_id: "gate-1".to_string(),
                gate_decision_id: "decision-1".to_string(),
                approved_at: datetime!(2026-05-26 08:15:00 UTC),
            },
            reason: "Replaced by a newer definition".to_string(),
        }
    }

    fn sample_outbox_event(
        event_id: &str,
        aggregate_id: &str,
        idempotency_key: Option<&str>,
    ) -> OutboxEvent {
        OutboxEvent::new_pending(
            event_id.to_string(),
            aggregate_id.to_string(),
            DefinitionEventEnvelope {
                event_id: event_id.to_string(),
                event_type: method_library_contracts::DefinitionEventType::ContentPublished,
                schema_version: "1.0".to_string(),
                occurred_at: datetime!(2026-05-26 08:10:00 UTC),
                producer: "L3-method-library".to_string(),
                content_ref: PublishedContentRef {
                    content_id: aggregate_id.to_string(),
                    kind: MethodContentKind::Qualification,
                    version: ContentVersion::new("1.0.0").expect("version should be valid"),
                    fingerprint: CanonicalFingerprint::new(
                        FingerprintAlgorithm::Sha256,
                        "abc123",
                        "1.0",
                    )
                    .expect("fingerprint should be valid"),
                },
                snapshot_ref: None,
                trace: EventTraceContext {
                    request_id: "req-1".to_string(),
                    trace_id: "trace-1".to_string(),
                },
                payload: method_library_contracts::DefinitionEventPayload::ContentPublished(
                    method_library_contracts::ContentPublishedPayload {
                        gate_ref: ApprovedGateRef {
                            gate_id: "gate-1".to_string(),
                            gate_decision_id: "decision-1".to_string(),
                            approved_at: datetime!(2026-05-26 08:05:00 UTC),
                        },
                        version: ContentVersion::new("1.0.0").expect("version should be valid"),
                        fingerprint: CanonicalFingerprint::new(
                            FingerprintAlgorithm::Sha256,
                            "abc123",
                            "1.0",
                        )
                        .expect("fingerprint should be valid"),
                    },
                ),
            },
            format!("payload-hash-{event_id}"),
            idempotency_key.map(ToString::to_string),
        )
        .expect("outbox event should be valid")
    }

    #[tokio::test]
    async fn unit_of_work_commits_and_rolls_back() {
        let _guard = test_guard();
        let persistence = test_db().await;
        let mut tx = persistence
            .unit_of_work()
            .begin(sample_meta())
            .await
            .expect("transaction should begin");
        let repository = persistence.method_content_repository();
        repository
            .insert(&mut tx, sample_content())
            .await
            .expect("insert should succeed");
        tx.commit().await.expect("commit should succeed");

        let mut read_tx = persistence
            .unit_of_work()
            .begin(sample_meta())
            .await
            .expect("transaction should begin");
        let content = repository
            .get_for_update(&mut read_tx, "content-1".to_string())
            .await
            .expect("query should work")
            .expect("content should exist");
        assert_eq!(content.content_id, "content-1");
        read_tx.rollback().await.expect("rollback should succeed");

        let mut rolled_back_content = sample_content();
        rolled_back_content.content_id = "content-2".to_string();
        rolled_back_content.content_family_id = "family-2".to_string();

        let mut tx = persistence
            .unit_of_work()
            .begin(sample_meta())
            .await
            .expect("transaction should begin");
        repository
            .insert(&mut tx, rolled_back_content)
            .await
            .expect("insert should succeed");
        tx.rollback().await.expect("rollback should succeed");

        let mut verify_tx = persistence
            .unit_of_work()
            .begin(sample_meta())
            .await
            .expect("transaction should begin");
        let rolled_back = repository
            .get_for_update(&mut verify_tx, "content-2".to_string())
            .await
            .expect("query should work");
        assert!(rolled_back.is_none());
        verify_tx.rollback().await.expect("rollback should succeed");
    }

    #[tokio::test]
    async fn repository_save_detects_revision_conflict() {
        let _guard = test_guard();
        let persistence = test_db().await;
        let repository = persistence.method_content_repository();
        let mut tx = persistence
            .unit_of_work()
            .begin(sample_meta())
            .await
            .expect("transaction should begin");
        let mut content = sample_content();
        repository
            .insert(&mut tx, content.clone())
            .await
            .expect("insert should succeed");
        content.revision = 2;
        let error = repository
            .save(&mut tx, content, 2)
            .await
            .expect_err("revision conflict should fail");
        assert_eq!(error.code, MethodLibraryErrorCode::RevisionConflict);
        tx.rollback().await.expect("rollback should succeed");
    }

    #[tokio::test]
    async fn version_repository_detects_unique_version_conflict() {
        let _guard = test_guard();
        let persistence = test_db().await;
        let repository = persistence.method_content_version_repository();
        let mut tx = persistence
            .unit_of_work()
            .begin(sample_meta())
            .await
            .expect("transaction should begin");
        repository
            .insert(
                &mut tx,
                sample_version_record("ver-1", "content-1", "family-1", "1.0.0"),
            )
            .await
            .expect("first version insert should succeed");

        let error = repository
            .insert(
                &mut tx,
                sample_version_record("ver-2", "content-2", "family-1", "1.0.0"),
            )
            .await
            .expect_err("duplicate family version should fail");
        assert_eq!(error.code, MethodLibraryErrorCode::ContentVersionConflict);
        tx.rollback().await.expect("rollback should succeed");
    }

    #[tokio::test]
    async fn repositories_support_read_only_query_methods() {
        let _guard = test_guard();
        let persistence = test_db().await;
        let content_repository = persistence.method_content_repository();
        let version_repository = persistence.method_content_version_repository();
        let mut tx = persistence
            .unit_of_work()
            .begin(sample_meta())
            .await
            .expect("transaction should begin");
        content_repository
            .insert(&mut tx, sample_content())
            .await
            .expect("content insert should succeed");
        version_repository
            .insert(
                &mut tx,
                sample_version_record("ver-1", "content-1", "family-1", "1.0.0"),
            )
            .await
            .expect("version insert should succeed");
        tx.commit().await.expect("commit should succeed");

        let content = content_repository
            .get("content-1".to_string())
            .await
            .expect("read-only content query should succeed")
            .expect("content should exist");
        assert_eq!(content.content_id, "content-1");

        let version = version_repository
            .get(
                "content-1".to_string(),
                ContentVersion::new("1.0.0").expect("version should be valid"),
            )
            .await
            .expect("read-only version query should succeed")
            .expect("version record should exist");
        assert_eq!(version.content_version_id, "ver-1");
    }

    #[tokio::test]
    async fn summary_projection_repository_filters_published_reads() {
        let _guard = test_guard();
        let persistence = test_db().await;
        let repository = persistence.content_summary_projection_repository();
        repository
            .upsert(sample_summary_view("content-draft", LifecycleState::Draft))
            .await
            .expect("draft summary should upsert");
        repository
            .upsert(sample_summary_view(
                "content-published",
                LifecycleState::Published,
            ))
            .await
            .expect("published summary should upsert");

        let published_items = repository
            .list(
                &ListMethodContentsQuery {
                    kind: Some(MethodContentKind::Qualification),
                    lifecycle_state: None,
                    read_mode: ReadMode::Published,
                    cursor: None,
                    limit: 10,
                    sort: Some("content_id".to_string()),
                },
                &PageRequest {
                    cursor: None,
                    limit: 10,
                },
            )
            .await
            .expect("published list query should succeed");
        assert_eq!(published_items.len(), 1);
        assert_eq!(published_items[0].content_id, "content-published");

        let authoring_items = repository
            .list(
                &ListMethodContentsQuery {
                    kind: Some(MethodContentKind::Qualification),
                    lifecycle_state: None,
                    read_mode: ReadMode::Authoring,
                    cursor: None,
                    limit: 10,
                    sort: Some("content_id".to_string()),
                },
                &PageRequest {
                    cursor: None,
                    limit: 10,
                },
            )
            .await
            .expect("authoring list query should succeed");
        assert_eq!(authoring_items.len(), 2);
    }

    #[tokio::test]
    async fn idempotency_repository_tracks_begin_and_completion() {
        let _guard = test_guard();
        let persistence = test_db().await;
        let repository = persistence.idempotency_repository();
        let mut tx = persistence
            .unit_of_work()
            .begin(sample_meta())
            .await
            .expect("transaction should begin");
        let started = repository
            .try_begin(
                &mut tx,
                "idem-1".to_string(),
                "command:create_draft".to_string(),
                "hash-1".to_string(),
                datetime!(2026-05-26 08:00:00 UTC),
            )
            .await
            .expect("idempotency should begin");
        assert_eq!(started, IdempotencyBeginResult::Started);
        repository
            .mark_completed(
                &mut tx,
                "idem-1".to_string(),
                "command:create_draft".to_string(),
                "result-1".to_string(),
                datetime!(2026-05-26 08:01:00 UTC),
            )
            .await
            .expect("idempotency should complete");
        tx.commit().await.expect("commit should succeed");
    }

    #[tokio::test]
    async fn outbox_repository_claims_and_marks_status() {
        let _guard = test_guard();
        let persistence = test_db().await;
        let outbox = persistence.outbox_repository();
        let mut tx = persistence
            .unit_of_work()
            .begin(sample_meta())
            .await
            .expect("transaction should begin");
        let event = OutboxEvent::new_pending(
            "evt-1".to_string(),
            "content-1".to_string(),
            DefinitionEventEnvelope {
                event_id: "evt-1".to_string(),
                event_type: method_library_contracts::DefinitionEventType::ContentPublished,
                schema_version: "1.0".to_string(),
                occurred_at: datetime!(2026-05-26 08:10:00 UTC),
                producer: "L3-method-library".to_string(),
                content_ref: PublishedContentRef {
                    content_id: "content-1".to_string(),
                    kind: MethodContentKind::Qualification,
                    version: ContentVersion::new("1.0.0").expect("version should be valid"),
                    fingerprint: CanonicalFingerprint::new(
                        FingerprintAlgorithm::Sha256,
                        "abc123",
                        "1.0",
                    )
                    .expect("fingerprint should be valid"),
                },
                snapshot_ref: None,
                trace: EventTraceContext {
                    request_id: "req-1".to_string(),
                    trace_id: "trace-1".to_string(),
                },
                payload: method_library_contracts::DefinitionEventPayload::ContentPublished(
                    method_library_contracts::ContentPublishedPayload {
                        gate_ref: ApprovedGateRef {
                            gate_id: "gate-1".to_string(),
                            gate_decision_id: "decision-1".to_string(),
                            approved_at: datetime!(2026-05-26 08:05:00 UTC),
                        },
                        version: ContentVersion::new("1.0.0").expect("version should be valid"),
                        fingerprint: CanonicalFingerprint::new(
                            FingerprintAlgorithm::Sha256,
                            "abc123",
                            "1.0",
                        )
                        .expect("fingerprint should be valid"),
                    },
                ),
            },
            "payload-hash-1".to_string(),
            Some("idem-1".to_string()),
        )
        .expect("outbox event should be valid");
        outbox
            .append(&mut tx, event)
            .await
            .expect("event should append");
        tx.commit().await.expect("commit should succeed");

        let claimed = outbox
            .claim_pending(
                10,
                "worker-1".to_string(),
                datetime!(2026-05-26 08:11:00 UTC),
                time::Duration::minutes(10),
            )
            .await
            .expect("event should claim");
        assert_eq!(claimed.len(), 1);
        assert_eq!(claimed[0].status, OutboxStatus::Publishing);
        assert_eq!(claimed[0].worker_id.as_deref(), Some("worker-1"));

        let empty_claim = outbox
            .claim_pending(
                10,
                "worker-2".to_string(),
                datetime!(2026-05-26 08:12:00 UTC),
                time::Duration::minutes(10),
            )
            .await
            .expect("lease should prevent a second claim");
        assert!(empty_claim.is_empty());

        outbox
            .mark_published(
                "evt-1".to_string(),
                "worker-1".to_string(),
                datetime!(2026-05-26 08:13:00 UTC),
            )
            .await
            .expect("publish should mark");
    }

    #[tokio::test]
    async fn publish_service_rolls_back_when_outbox_idempotency_conflicts() {
        let _guard = test_guard();
        let persistence = test_db().await;
        insert_content(
            &persistence,
            sample_in_review_content("content-review", "family-review"),
            "idem-seed-review",
        )
        .await;

        let outbox = persistence.outbox_repository();
        let mut tx = persistence
            .unit_of_work()
            .begin(sample_meta_with("idem-seed-outbox", "hash-seed-outbox"))
            .await
            .expect("transaction should begin");
        outbox
            .append(
                &mut tx,
                sample_outbox_event("evt-existing", "content-existing", Some("idem-publish")),
            )
            .await
            .expect("seed event should append");
        tx.commit().await.expect("commit should succeed");

        let service = command_service(&persistence);
        let error = service
            .publish(
                sample_publish_command("content-review", 2),
                sample_actor(),
                sample_meta_with("idem-publish", "hash-publish"),
            )
            .await
            .expect_err("publish should roll back on outbox idempotency conflict");
        assert_eq!(error.code, MethodLibraryErrorCode::IdempotencyConflict);

        let repository = persistence.method_content_repository();
        let mut read_tx = persistence
            .unit_of_work()
            .begin(sample_meta_with("idem-read", "hash-read"))
            .await
            .expect("transaction should begin");
        let content = repository
            .get_for_update(&mut read_tx, "content-review".to_string())
            .await
            .expect("content query should succeed")
            .expect("content should exist");
        assert_eq!(content.lifecycle.state, LifecycleState::InReview);
        assert!(content.version.is_none());
        assert!(content.fingerprint.is_none());
        read_tx.rollback().await.expect("rollback should succeed");

        let version_count: i64 = sqlx::query_scalar(&format!(
            "select count(*) from {METHOD_CONTENT_VERSIONS_TABLE} where content_id = $1"
        ))
        .bind("content-review")
        .fetch_one(&persistence.state.pool)
        .await
        .expect("version count should query");
        assert_eq!(version_count, 0);

        let snapshot_count: i64 = sqlx::query_scalar(&format!(
            "select count(*) from {SNAPSHOT_TABLE} where content_id = $1"
        ))
        .bind("content-review")
        .fetch_one(&persistence.state.pool)
        .await
        .expect("snapshot count should query");
        assert_eq!(snapshot_count, 0);
    }

    #[tokio::test]
    async fn supersede_service_rolls_back_when_link_conflicts() {
        let _guard = test_guard();
        let persistence = test_db().await;
        insert_content(
            &persistence,
            sample_published_content("content-old", "family-shared"),
            "idem-seed-old",
        )
        .await;
        insert_content(
            &persistence,
            sample_in_review_content("content-new", "family-new"),
            "idem-seed-new",
        )
        .await;

        let links = persistence.supersede_link_repository();
        let mut tx = persistence
            .unit_of_work()
            .begin(sample_meta_with("idem-seed-link", "hash-seed-link"))
            .await
            .expect("transaction should begin");
        links
            .insert(
                &mut tx,
                SupersedeLink {
                    supersede_link_id: "supersede-existing".to_string(),
                    old_content_id: "content-old".to_string(),
                    new_content_id: "content-other".to_string(),
                    content_family_id: "family-shared".to_string(),
                    reason: "Existing replacement".to_string(),
                    created_at: datetime!(2026-05-26 08:12:00 UTC),
                },
            )
            .await
            .expect("seed link should insert");
        tx.commit().await.expect("commit should succeed");

        let service = command_service(&persistence);
        let error = service
            .supersede(
                sample_supersede_command("content-old", 3, "content-new", 2),
                sample_actor(),
                sample_meta_with("idem-supersede", "hash-supersede"),
            )
            .await
            .expect_err("supersede should roll back on conflicting link");
        assert_eq!(error.code, MethodLibraryErrorCode::SupersedeConflict);

        let repository = persistence.method_content_repository();
        let mut read_tx = persistence
            .unit_of_work()
            .begin(sample_meta_with("idem-read-2", "hash-read-2"))
            .await
            .expect("transaction should begin");
        let old_content = repository
            .get_for_update(&mut read_tx, "content-old".to_string())
            .await
            .expect("old query should succeed")
            .expect("old content should exist");
        let new_content = repository
            .get_for_update(&mut read_tx, "content-new".to_string())
            .await
            .expect("new query should succeed")
            .expect("new content should exist");
        assert_eq!(old_content.lifecycle.state, LifecycleState::Published);
        assert!(old_content.superseded_by_content_id.is_none());
        assert_eq!(new_content.lifecycle.state, LifecycleState::InReview);
        assert_eq!(new_content.content_family_id, "family-new");
        assert!(new_content.version.is_none());
        read_tx.rollback().await.expect("rollback should succeed");

        let version_count: i64 = sqlx::query_scalar(&format!(
            "select count(*) from {METHOD_CONTENT_VERSIONS_TABLE} where content_id = $1"
        ))
        .bind("content-new")
        .fetch_one(&persistence.state.pool)
        .await
        .expect("version count should query");
        assert_eq!(version_count, 0);

        let snapshot_count: i64 = sqlx::query_scalar(&format!(
            "select count(*) from {SNAPSHOT_TABLE} where content_id = $1"
        ))
        .bind("content-new")
        .fetch_one(&persistence.state.pool)
        .await
        .expect("snapshot count should query");
        assert_eq!(snapshot_count, 0);
    }

    #[tokio::test]
    async fn projection_checkpoint_uses_compare_and_swap() {
        let _guard = test_guard();
        let persistence = test_db().await;
        let checkpoints = persistence.projection_checkpoint_repository();
        checkpoints
            .advance_if_current(
                "summary".to_string(),
                None,
                "evt-1".to_string(),
                datetime!(2026-05-26 08:00:00 UTC),
            )
            .await
            .expect("initial checkpoint should insert");
        let error = checkpoints
            .advance_if_current(
                "summary".to_string(),
                Some("evt-stale".to_string()),
                "evt-2".to_string(),
                datetime!(2026-05-26 08:01:00 UTC),
            )
            .await
            .expect_err("stale checkpoint should fail");
        assert_eq!(error.code, MethodLibraryErrorCode::CheckpointConflict);
    }
}

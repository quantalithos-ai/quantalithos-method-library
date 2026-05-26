create table method_contents (
    content_id text primary key,
    content_family_id text not null,
    kind text not null,
    name text not null,
    description text,
    lifecycle_state text not null,
    version_text text,
    fingerprint_value text,
    supersedes_content_id text,
    superseded_by_content_id text,
    created_by text not null,
    created_at timestamptz not null,
    updated_at timestamptz not null,
    revision bigint not null,
    content_json jsonb not null
);

create index method_contents_kind_state_updated_idx
    on method_contents (kind, lifecycle_state, updated_at);

create index method_contents_family_idx
    on method_contents (content_family_id);

create index method_contents_fingerprint_idx
    on method_contents (fingerprint_value);

create table method_content_references (
    reference_id bigserial primary key,
    source_content_id text not null,
    reference_kind text not null,
    target_content_id text not null,
    target_kind text not null,
    required_state text not null,
    target_version_text text,
    target_fingerprint_value text,
    reference_json jsonb not null,
    created_at timestamptz not null default now()
);

create index method_content_references_source_kind_idx
    on method_content_references (source_content_id, reference_kind);

create unique index method_content_references_draft_unique_idx
    on method_content_references (source_content_id, target_content_id)
    where reference_kind = 'draft';

create unique index method_content_references_published_unique_idx
    on method_content_references (source_content_id, target_content_id, target_version_text)
    where reference_kind = 'published';

create table method_content_versions (
    content_version_id text primary key,
    content_id text not null,
    content_family_id text not null,
    version_text text not null,
    fingerprint_value text not null,
    snapshot_id text not null,
    published_at timestamptz not null,
    version_json jsonb not null,
    unique (content_family_id, version_text),
    unique (content_id, version_text)
);

create index method_content_versions_content_idx
    on method_content_versions (content_id, published_at);

create table supersede_links (
    supersede_link_id text primary key,
    old_content_id text not null unique,
    new_content_id text not null,
    reason text not null,
    created_at timestamptz not null
);

create index supersede_links_new_content_idx
    on supersede_links (new_content_id);

create table lifecycle_history_entries (
    history_entry_id text primary key,
    content_id text not null,
    from_state text,
    to_state text not null,
    actor_id text not null,
    reason text,
    created_at timestamptz not null,
    entry_json jsonb not null
);

create index lifecycle_history_entries_content_created_idx
    on lifecycle_history_entries (content_id, created_at);

create table audit_records (
    audit_id text primary key,
    request_id text not null,
    trace_id text not null,
    actor_context_json jsonb not null,
    target_ref_json jsonb not null,
    action text not null,
    result text not null,
    details_json jsonb not null,
    occurred_at timestamptz not null,
    record_json jsonb not null
);

create index audit_records_trace_action_occurred_idx
    on audit_records (trace_id, action, occurred_at);

create table outbox_events (
    outbox_event_id text primary key,
    aggregate_id text not null,
    event_type text not null,
    payload_json jsonb not null,
    payload_hash text not null,
    status text not null,
    retry_count integer not null check (retry_count >= 0),
    next_retry_at timestamptz,
    worker_id text,
    lease_until timestamptz,
    published_at timestamptz,
    idempotency_key text unique
);

create index outbox_events_status_retry_idx
    on outbox_events (status, next_retry_at);

create index outbox_events_aggregate_idx
    on outbox_events (aggregate_id);

create index outbox_events_event_type_idx
    on outbox_events (event_type);

create table idempotency_records (
    scope text not null,
    idempotency_key text not null,
    request_hash text not null,
    status text not null,
    result_ref text,
    failure_reason_json jsonb,
    updated_at timestamptz not null,
    primary key (scope, idempotency_key)
);

create index idempotency_records_scope_status_updated_idx
    on idempotency_records (scope, status, updated_at);

create table definition_snapshots (
    snapshot_id text primary key,
    content_id text not null,
    version_text text not null,
    fingerprint_value text not null,
    schema_version text not null,
    blob_ref text not null,
    created_at timestamptz not null,
    content_ref_json jsonb not null,
    references_json jsonb not null,
    snapshot_json jsonb not null,
    unique (content_id, version_text, fingerprint_value)
);

create index definition_snapshots_content_version_idx
    on definition_snapshots (content_id, version_text);

create table content_summary_projection (
    content_id text primary key,
    kind text not null,
    name text not null,
    lifecycle_state text not null,
    version_text text,
    fingerprint_value text,
    updated_at timestamptz not null,
    view_json jsonb not null
);

create index content_summary_projection_list_idx
    on content_summary_projection (kind, lifecycle_state, updated_at);

create table definition_trace_projection (
    content_id text primary key,
    trace_json jsonb not null,
    updated_at timestamptz not null
);

create index definition_trace_projection_updated_idx
    on definition_trace_projection (updated_at);

create table projection_checkpoints (
    checkpoint_name text primary key,
    last_processed_event_id text,
    status text not null,
    updated_at timestamptz not null
);

create index projection_checkpoints_status_updated_idx
    on projection_checkpoints (status, updated_at);

create table inbound_dead_letters (
    dead_letter_id text primary key,
    source_module text not null,
    event_type text not null,
    payload_json jsonb not null,
    failure_reason_json jsonb not null,
    replay_status text not null default 'pending',
    created_at timestamptz not null
);

create index inbound_dead_letters_source_event_idx
    on inbound_dead_letters (source_module, event_type, replay_status);

create table job_runs (
    job_run_id text primary key,
    job_name text not null,
    scope_hash text not null,
    idempotency_key text not null,
    status text not null,
    result_json jsonb,
    started_at timestamptz not null,
    finished_at timestamptz,
    unique (job_name, scope_hash, idempotency_key)
);

create index job_runs_name_status_started_idx
    on job_runs (job_name, status, started_at);

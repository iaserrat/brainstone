-- The durable runtime schema, one baseline. There is no migration history:
-- a database is either this schema or unsupported (see docs/operations.md).
CREATE TABLE sessions (
    id BIGSERIAL PRIMARY KEY,
    surface TEXT NOT NULL,
    external_id TEXT NOT NULL,
    lease_owner TEXT,
    lease_token BIGINT NOT NULL DEFAULT 0,
    lease_expires_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT transaction_timestamp(),
    UNIQUE (surface, external_id)
);

CREATE TABLE lanes (
    id BIGSERIAL PRIMARY KEY,
    session_id BIGINT NOT NULL REFERENCES sessions(id),
    external_id TEXT NOT NULL,
    system_prompt TEXT NOT NULL,
    tool_definitions JSONB NOT NULL DEFAULT '[]'::jsonb,
    execution_profile JSONB NOT NULL
        DEFAULT '{"model":null,"effort":null,"max_turns":25,"depth":0}'::jsonb,
    execution_identity JSONB,
    follow_enabled BOOLEAN NOT NULL DEFAULT TRUE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT transaction_timestamp(),
    UNIQUE (session_id, external_id)
);

CREATE TABLE operations (
    id BIGSERIAL PRIMARY KEY,
    lane_id BIGINT NOT NULL REFERENCES lanes(id),
    request_id TEXT NOT NULL,
    prompt TEXT NOT NULL,
    source_prompt TEXT,
    state TEXT NOT NULL CHECK (state IN ('accepted', 'running', 'completed', 'failed')),
    outcome JSONB,
    lease_token BIGINT,
    next_event_sequence BIGINT NOT NULL DEFAULT 0,
    delivery_target JSONB,
    actor TEXT,
    execution_identity JSONB,
    schedule_id BIGINT,
    image_count SMALLINT NOT NULL CHECK (image_count >= 0),
    image_claim_token BIGINT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT transaction_timestamp(),
    UNIQUE (lane_id, request_id),
    CONSTRAINT operations_image_claim_capable
        CHECK (state <> 'running' OR image_count = 0
            OR image_claim_token IS NOT DISTINCT FROM lease_token)
);

CREATE INDEX operations_pending ON operations (id) WHERE state IN ('accepted', 'running');

CREATE INDEX operations_actor ON operations (actor, created_at)
    WHERE actor IS NOT NULL;

CREATE INDEX operations_delivery_representative
    ON operations (lane_id, (delivery_target->>'adapter'), id)
    WHERE state IN ('accepted', 'running');

CREATE TABLE operation_events (
    operation_id BIGINT NOT NULL REFERENCES operations(id),
    sequence BIGINT NOT NULL,
    schema_version SMALLINT NOT NULL,
    payload JSONB NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT transaction_timestamp(),
    PRIMARY KEY (operation_id, sequence)
);

CREATE TABLE lane_compactions (
    id BIGSERIAL PRIMARY KEY,
    lane_id BIGINT NOT NULL REFERENCES lanes(id),
    through_operation_id BIGINT NOT NULL REFERENCES operations(id),
    summary TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT transaction_timestamp(),
    UNIQUE (lane_id, through_operation_id)
);

CREATE TABLE operation_deliveries (
    operation_id BIGINT PRIMARY KEY REFERENCES operations(id),
    next_event_sequence BIGINT NOT NULL DEFAULT 0,
    external_id TEXT,
    lease_owner TEXT,
    lease_token BIGINT NOT NULL DEFAULT 0,
    lease_expires_at TIMESTAMPTZ,
    presented_at TIMESTAMPTZ,
    settled BOOLEAN NOT NULL DEFAULT FALSE
);

CREATE INDEX operation_deliveries_pending
    ON operation_deliveries (operation_id, next_event_sequence);

CREATE INDEX operation_deliveries_claimable
    ON operation_deliveries (operation_id)
    WHERE NOT settled;

CREATE TABLE interactions (
    id TEXT PRIMARY KEY,
    operation_id BIGINT NOT NULL REFERENCES operations(id),
    request JSONB NOT NULL,
    client_message_id UUID NOT NULL DEFAULT gen_random_uuid(),
    external_id TEXT,
    response JSONB,
    created_at TIMESTAMPTZ NOT NULL DEFAULT transaction_timestamp(),
    answered_at TIMESTAMPTZ
);

CREATE INDEX interactions_operation ON interactions (operation_id);

CREATE TABLE subagent_runs (
    id BIGSERIAL PRIMARY KEY,
    parent_operation_id BIGINT NOT NULL REFERENCES operations(id),
    root_session_id BIGINT NOT NULL REFERENCES sessions(id),
    child_session_id BIGINT UNIQUE REFERENCES sessions(id),
    spawn_key TEXT NOT NULL,
    request JSONB NOT NULL,
    child_operation_id BIGINT UNIQUE REFERENCES operations(id),
    created_at TIMESTAMPTZ NOT NULL DEFAULT transaction_timestamp(),
    UNIQUE (parent_operation_id, spawn_key)
);

CREATE INDEX subagent_runs_active_root_idx ON subagent_runs (root_session_id);

CREATE TABLE operation_steering (
    id BIGSERIAL PRIMARY KEY,
    lane_id BIGINT NOT NULL REFERENCES lanes(id),
    operation_id BIGINT NOT NULL REFERENCES operations(id),
    request_id TEXT NOT NULL,
    prompt TEXT NOT NULL,
    source_prompt TEXT,
    delivery_target JSONB,
    execution_identity JSONB,
    image_count SMALLINT NOT NULL CHECK (image_count >= 0),
    consumed BOOLEAN NOT NULL DEFAULT FALSE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT transaction_timestamp(),
    UNIQUE (lane_id, request_id)
);

CREATE INDEX operation_steering_pending
    ON operation_steering (operation_id, id) WHERE consumed = FALSE;

CREATE TABLE memories (
    id BIGSERIAL PRIMARY KEY,
    scope TEXT NOT NULL,
    idempotency_key TEXT NOT NULL UNIQUE,
    content TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT transaction_timestamp()
);

CREATE INDEX memories_scope ON memories (scope, id DESC);

CREATE TABLE lane_follow_activations (
    lane_id BIGINT NOT NULL REFERENCES lanes(id),
    request_id TEXT NOT NULL,
    PRIMARY KEY (lane_id, request_id)
);

CREATE TABLE lane_unfollow_calls (
    call_key TEXT PRIMARY KEY,
    lane_id BIGINT NOT NULL REFERENCES lanes(id)
);

CREATE TABLE operation_images (
    operation_id BIGINT NOT NULL REFERENCES operations(id),
    ordinal SMALLINT NOT NULL CHECK (ordinal >= 0),
    media_type TEXT NOT NULL CHECK (media_type IN ('image/jpeg', 'image/png', 'image/webp')),
    bytes BYTEA NOT NULL CHECK (octet_length(bytes) > 0),
    width INTEGER NOT NULL CHECK (width > 0),
    height INTEGER NOT NULL CHECK (height > 0),
    PRIMARY KEY (operation_id, ordinal)
);

CREATE TABLE operation_steering_images (
    steering_id BIGINT NOT NULL REFERENCES operation_steering(id),
    ordinal SMALLINT NOT NULL CHECK (ordinal >= 0),
    media_type TEXT NOT NULL CHECK (media_type IN ('image/jpeg', 'image/png', 'image/webp')),
    bytes BYTEA NOT NULL CHECK (octet_length(bytes) > 0),
    width INTEGER NOT NULL CHECK (width > 0),
    height INTEGER NOT NULL CHECK (height > 0),
    PRIMARY KEY (steering_id, ordinal)
);

CREATE TABLE scheduled_work (
    id BIGSERIAL PRIMARY KEY,
    origin_operation_id BIGINT NOT NULL REFERENCES operations(id),
    lane_id BIGINT NOT NULL REFERENCES lanes(id),
    schedule_key TEXT NOT NULL,
    prompt TEXT NOT NULL,
    run_at_unix_ms BIGINT NOT NULL,
    schedule_kind TEXT NOT NULL CHECK (schedule_kind IN ('at', 'after', 'repeat', 'recur')),
    schedule_value BIGINT NOT NULL,
    state TEXT NOT NULL CHECK (state IN ('pending', 'cancelled', 'released')),
    delivery_target JSONB,
    released_operation_id BIGINT UNIQUE REFERENCES operations(id),
    repeat_minute SMALLINT CHECK (repeat_minute BETWEEN 0 AND 1439),
    repeat_time_zone TEXT,
    repeat_days SMALLINT[],
    consecutive_failures SMALLINT NOT NULL DEFAULT 0,
    recurrence JSONB,
    destination JSONB,
    created_at TIMESTAMPTZ NOT NULL DEFAULT transaction_timestamp(),
    UNIQUE (origin_operation_id, schedule_key),
    CONSTRAINT scheduled_work_repeat CHECK (
        (schedule_kind = 'repeat') = (repeat_minute IS NOT NULL)
        AND (repeat_minute IS NULL) = (repeat_time_zone IS NULL)
        AND (repeat_days IS NULL OR repeat_minute IS NOT NULL)),
    CONSTRAINT scheduled_work_recur
        CHECK ((schedule_kind = 'recur') = (recurrence IS NOT NULL)),
    CONSTRAINT scheduled_work_standing_never_released
        CHECK (schedule_kind NOT IN ('repeat', 'recur') OR state <> 'released')
);

CREATE INDEX scheduled_work_due
    ON scheduled_work (run_at_unix_ms, id) WHERE state = 'pending';

ALTER TABLE operations ADD CONSTRAINT operations_schedule_id_fkey
    FOREIGN KEY (schedule_id) REFERENCES scheduled_work(id);

CREATE TABLE tool_media (
    root_session_id BIGINT NOT NULL REFERENCES sessions(id),
    reference TEXT NOT NULL CHECK (reference ~ '^(image|video|file):[0-9a-f]{16}$'),
    media_type TEXT NOT NULL,
    bytes BYTEA NOT NULL CHECK (octet_length(bytes) > 0),
    width INTEGER CHECK (width > 0),
    height INTEGER CHECK (height > 0),
    name TEXT CHECK (octet_length(name) BETWEEN 1 AND 255),
    stored_at TIMESTAMPTZ NOT NULL DEFAULT transaction_timestamp(),
    PRIMARY KEY (root_session_id, reference),
    CONSTRAINT tool_media_kind_check CHECK (CASE split_part(reference, ':', 1)
        WHEN 'image' THEN media_type IN ('image/jpeg', 'image/png', 'image/webp')
            AND width IS NOT NULL AND height IS NOT NULL AND name IS NULL
        WHEN 'video' THEN media_type IN ('video/mp4', 'video/quicktime')
            AND width IS NULL AND height IS NULL AND name IS NULL
        ELSE media_type ~ '^[a-z0-9!#$&^_.+-]+/[a-z0-9!#$&^_.+-]+$'
            AND width IS NULL AND height IS NULL AND name IS NOT NULL
    END)
);

CREATE INDEX tool_media_stored_at ON tool_media (stored_at);

ALTER TABLE tool_media ALTER COLUMN bytes SET STORAGE EXTERNAL;

CREATE TABLE tool_approvals (
    id TEXT PRIMARY KEY,
    operation_id BIGINT NOT NULL REFERENCES operations(id),
    request TEXT NOT NULL,
    decision TEXT CHECK (decision IN ('approved', 'denied')),
    actor TEXT,
    expires_at_unix_ms BIGINT NOT NULL
);

CREATE INDEX tool_approvals_pending ON tool_approvals (operation_id, id) WHERE decision IS NULL;

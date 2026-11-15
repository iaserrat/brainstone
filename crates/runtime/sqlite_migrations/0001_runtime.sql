CREATE TABLE sessions (
 id INTEGER PRIMARY KEY AUTOINCREMENT, surface TEXT NOT NULL, external_id TEXT NOT NULL,
 lease_owner TEXT, lease_token INTEGER NOT NULL DEFAULT 0, lease_expires_at INTEGER,
 created_at INTEGER NOT NULL DEFAULT (CAST(unixepoch('subsec') * 1000 AS INTEGER)),
 UNIQUE(surface, external_id)
);
CREATE TABLE lanes (
 id INTEGER PRIMARY KEY AUTOINCREMENT, session_id INTEGER NOT NULL REFERENCES sessions(id),
 external_id TEXT NOT NULL, system_prompt TEXT NOT NULL, tool_definitions TEXT NOT NULL CHECK(tool_definitions IS NULL OR json_valid(tool_definitions)) DEFAULT '[]',
 execution_profile TEXT NOT NULL CHECK(execution_profile IS NULL OR json_valid(execution_profile)), execution_identity TEXT CHECK(execution_identity IS NULL OR json_valid(execution_identity)), follow_enabled INTEGER NOT NULL DEFAULT 1,
 created_at INTEGER NOT NULL DEFAULT (CAST(unixepoch('subsec') * 1000 AS INTEGER)),
 UNIQUE(session_id, external_id)
);
CREATE TABLE operations (
 id INTEGER PRIMARY KEY AUTOINCREMENT, lane_id INTEGER NOT NULL REFERENCES lanes(id),
 request_id TEXT NOT NULL, prompt TEXT NOT NULL, source_prompt TEXT NOT NULL,
 state TEXT NOT NULL CHECK(state IN ('accepted','running','completed','failed')), outcome TEXT CHECK(outcome IS NULL OR json_valid(outcome)),
 lease_token INTEGER, next_event_sequence INTEGER NOT NULL DEFAULT 0,
 delivery_target TEXT CHECK(delivery_target IS NULL OR json_valid(delivery_target)), actor TEXT, execution_identity TEXT CHECK(execution_identity IS NULL OR json_valid(execution_identity)), schedule_id INTEGER REFERENCES scheduled_work(id),
 image_count INTEGER NOT NULL CHECK(image_count >= 0), image_claim_token INTEGER,
 created_at INTEGER NOT NULL DEFAULT (CAST(unixepoch('subsec') * 1000 AS INTEGER)),
 UNIQUE(lane_id, request_id),
 CHECK(state <> 'running' OR image_count = 0 OR image_claim_token IS lease_token)
);
CREATE INDEX operations_pending ON operations(id) WHERE state IN ('accepted','running');
CREATE INDEX operations_lane ON operations(lane_id,id);
CREATE INDEX operations_actor ON operations(actor,created_at);
CREATE TABLE operation_events (
 operation_id INTEGER NOT NULL REFERENCES operations(id), sequence INTEGER NOT NULL,
 schema_version INTEGER NOT NULL, payload TEXT NOT NULL CHECK(payload IS NULL OR json_valid(payload)),
 created_at INTEGER NOT NULL DEFAULT (CAST(unixepoch('subsec') * 1000 AS INTEGER)),
 PRIMARY KEY(operation_id,sequence)
);
CREATE TABLE lane_compactions (
 id INTEGER PRIMARY KEY AUTOINCREMENT, lane_id INTEGER NOT NULL REFERENCES lanes(id),
 through_operation_id INTEGER NOT NULL REFERENCES operations(id), summary TEXT NOT NULL,
 created_at INTEGER NOT NULL DEFAULT (CAST(unixepoch('subsec') * 1000 AS INTEGER)),
 UNIQUE(lane_id,through_operation_id)
);
CREATE TABLE operation_deliveries (
 operation_id INTEGER PRIMARY KEY REFERENCES operations(id), next_event_sequence INTEGER NOT NULL DEFAULT 0,
 external_id TEXT, lease_owner TEXT, lease_token INTEGER NOT NULL DEFAULT 0,
 lease_expires_at INTEGER, presented_at INTEGER, settled INTEGER NOT NULL DEFAULT 0
);
CREATE INDEX deliveries_pending ON operation_deliveries(operation_id) WHERE NOT settled;
CREATE TABLE interactions (
 id TEXT PRIMARY KEY, operation_id INTEGER NOT NULL REFERENCES operations(id), request TEXT NOT NULL CHECK(request IS NULL OR json_valid(request)),
 client_message_id TEXT NOT NULL, external_id TEXT, response TEXT CHECK(response IS NULL OR json_valid(response)),
 created_at INTEGER NOT NULL DEFAULT (CAST(unixepoch('subsec') * 1000 AS INTEGER)), answered_at INTEGER
);
CREATE INDEX interactions_operation ON interactions(operation_id);
CREATE TABLE subagent_runs (
 id INTEGER PRIMARY KEY AUTOINCREMENT, parent_operation_id INTEGER NOT NULL REFERENCES operations(id),
 root_session_id INTEGER NOT NULL REFERENCES sessions(id), child_session_id INTEGER UNIQUE REFERENCES sessions(id),
 spawn_key TEXT NOT NULL, request TEXT NOT NULL CHECK(request IS NULL OR json_valid(request)), child_operation_id INTEGER UNIQUE REFERENCES operations(id),
 created_at INTEGER NOT NULL DEFAULT (CAST(unixepoch('subsec') * 1000 AS INTEGER)),
 UNIQUE(parent_operation_id,spawn_key)
);
CREATE INDEX subagents_root ON subagent_runs(root_session_id);
CREATE TABLE operation_steering (
 id INTEGER PRIMARY KEY AUTOINCREMENT, lane_id INTEGER NOT NULL REFERENCES lanes(id),
 operation_id INTEGER NOT NULL REFERENCES operations(id), request_id TEXT NOT NULL,
 prompt TEXT NOT NULL, source_prompt TEXT NOT NULL, delivery_target TEXT CHECK(delivery_target IS NULL OR json_valid(delivery_target)), consumed INTEGER NOT NULL DEFAULT 0, execution_identity TEXT CHECK(execution_identity IS NULL OR json_valid(execution_identity)),
 image_count INTEGER NOT NULL CHECK(image_count >= 0),
 created_at INTEGER NOT NULL DEFAULT (CAST(unixepoch('subsec') * 1000 AS INTEGER)),
 UNIQUE(lane_id,request_id)
);
CREATE INDEX steering_pending ON operation_steering(operation_id,id) WHERE NOT consumed;
CREATE TABLE scheduled_work (
 id INTEGER PRIMARY KEY AUTOINCREMENT, origin_operation_id INTEGER NOT NULL REFERENCES operations(id),
 lane_id INTEGER NOT NULL REFERENCES lanes(id), schedule_key TEXT NOT NULL, prompt TEXT NOT NULL,
 run_at_unix_ms INTEGER NOT NULL, schedule_kind TEXT NOT NULL CHECK(schedule_kind IN ('at','after','repeat','recur')),
 schedule_value INTEGER NOT NULL, state TEXT NOT NULL CHECK(state IN ('pending','cancelled','released')),
 delivery_target TEXT CHECK(delivery_target IS NULL OR json_valid(delivery_target)), released_operation_id INTEGER UNIQUE REFERENCES operations(id),
 repeat_minute INTEGER CHECK(repeat_minute BETWEEN 0 AND 1439), repeat_time_zone TEXT, repeat_days TEXT CHECK(repeat_days IS NULL OR json_valid(repeat_days)),
 consecutive_failures INTEGER NOT NULL DEFAULT 0, recurrence TEXT CHECK(recurrence IS NULL OR json_valid(recurrence)), destination TEXT CHECK(destination IS NULL OR json_valid(destination)),
 created_at INTEGER NOT NULL DEFAULT (CAST(unixepoch('subsec') * 1000 AS INTEGER)),
 UNIQUE(origin_operation_id,schedule_key)
);
CREATE INDEX schedules_due ON scheduled_work(run_at_unix_ms,id) WHERE state='pending';
CREATE INDEX schedules_lane ON scheduled_work(lane_id);
CREATE TABLE memories (
 id INTEGER PRIMARY KEY AUTOINCREMENT, scope TEXT NOT NULL, idempotency_key TEXT NOT NULL UNIQUE,
 content TEXT NOT NULL, created_at INTEGER NOT NULL DEFAULT (CAST(unixepoch('subsec') * 1000 AS INTEGER))
);
CREATE INDEX memories_scope ON memories(scope,id DESC);
CREATE TABLE lane_follow_activations (
 lane_id INTEGER NOT NULL REFERENCES lanes(id), request_id TEXT NOT NULL, PRIMARY KEY(lane_id,request_id)
);
CREATE TABLE lane_unfollow_calls (
 call_key TEXT PRIMARY KEY, lane_id INTEGER NOT NULL REFERENCES lanes(id)
);
CREATE TRIGGER follow_activation AFTER INSERT ON lane_follow_activations
 BEGIN UPDATE lanes SET follow_enabled=1 WHERE id=NEW.lane_id; END;
CREATE TRIGGER unfollow_call AFTER INSERT ON lane_unfollow_calls
 BEGIN UPDATE lanes SET follow_enabled=0 WHERE id=NEW.lane_id; END;
CREATE TRIGGER event_delivery AFTER INSERT ON operation_events
 BEGIN UPDATE operation_deliveries SET presented_at=NULL,lease_owner=NULL,lease_expires_at=NULL,
 lease_token=lease_token+1 WHERE operation_id=NEW.operation_id AND next_event_sequence=NEW.sequence
 AND lease_owner IS NOT NULL; END;
CREATE TABLE operation_images (operation_id INTEGER NOT NULL REFERENCES operations(id), ordinal INTEGER NOT NULL CHECK(ordinal>=0), media_type TEXT NOT NULL CHECK(media_type IN ('image/jpeg','image/png','image/webp')), bytes BLOB NOT NULL CHECK(length(bytes)>0), width INTEGER NOT NULL CHECK(width>0), height INTEGER NOT NULL CHECK(height>0), PRIMARY KEY(operation_id,ordinal));
CREATE TABLE operation_steering_images (steering_id INTEGER NOT NULL REFERENCES operation_steering(id), ordinal INTEGER NOT NULL CHECK(ordinal>=0), media_type TEXT NOT NULL CHECK(media_type IN ('image/jpeg','image/png','image/webp')), bytes BLOB NOT NULL CHECK(length(bytes)>0), width INTEGER NOT NULL CHECK(width>0), height INTEGER NOT NULL CHECK(height>0), PRIMARY KEY(steering_id,ordinal));
CREATE TABLE tool_media (
 root_session_id INTEGER NOT NULL REFERENCES sessions(id), reference TEXT NOT NULL,
 media_type TEXT NOT NULL, bytes BLOB NOT NULL CHECK(length(bytes)>0),
 width INTEGER CHECK(width>0), height INTEGER CHECK(height>0), name TEXT,
 stored_at INTEGER NOT NULL DEFAULT (CAST(unixepoch('subsec') * 1000 AS INTEGER)),
 PRIMARY KEY(root_session_id,reference),
 CHECK(substr(reference,1,instr(reference,':')-1) IN ('image','video','file')),
 CHECK(length(substr(reference,instr(reference,':')+1))=16
       AND substr(reference,instr(reference,':')+1) NOT GLOB '*[^0-9a-f]*'),
 CHECK(CASE substr(reference,1,instr(reference,':')-1)
   WHEN 'image' THEN media_type IN ('image/jpeg','image/png','image/webp')
     AND width IS NOT NULL AND height IS NOT NULL AND name IS NULL
   WHEN 'video' THEN media_type IN ('video/mp4','video/quicktime')
     AND width IS NULL AND height IS NULL AND name IS NULL
   ELSE width IS NULL AND height IS NULL AND name IS NOT NULL
     AND length(CAST(name AS BLOB)) BETWEEN 1 AND 255 END)
);
CREATE INDEX tool_media_age ON tool_media(stored_at);
CREATE TABLE tool_approvals (
 id TEXT PRIMARY KEY, operation_id INTEGER NOT NULL REFERENCES operations(id),
 request TEXT NOT NULL CHECK(json_valid(request)), decision TEXT CHECK(decision IN ('approved','denied')), actor TEXT,
 expires_at_unix_ms INTEGER NOT NULL
);
CREATE INDEX tool_approvals_pending ON tool_approvals(operation_id,id) WHERE decision IS NULL;


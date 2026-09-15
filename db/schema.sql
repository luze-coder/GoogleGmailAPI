PRAGMA foreign_keys = ON;

------------------------------------------------------
-- STATUS
------------------------------------------------------

CREATE TABLE IF NOT EXISTS status (
    id INTEGER PRIMARY KEY,

    name TEXT NOT NULL UNIQUE
        CHECK(name IN ('PENDING', 'PROCESSING', 'COMPLETED', 'FAILED'))
);

INSERT OR IGNORE INTO status (id, name) VALUES
    (1, 'PENDING'),
    (2, 'PROCESSING'),
    (3, 'COMPLETED'),
    (4, 'FAILED');

------------------------------------------------------
-- TASK QUEUE
------------------------------------------------------

CREATE TABLE IF NOT EXISTS task_queue (
    id INTEGER PRIMARY KEY AUTOINCREMENT,

    worker_type TEXT NOT NULL,

    endpoint TEXT NOT NULL,

    parameters_json TEXT
        CHECK(parameters_json IS NULL OR json_valid(parameters_json)),

    status INTEGER NOT NULL
        REFERENCES status(id),

    locked_by TEXT,

    locked_at DATETIME,

    created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,

    completed_at DATETIME
);


------------------------------------------------------
-- ASSETS
------------------------------------------------------

CREATE TABLE IF NOT EXISTS assets (
    id INTEGER PRIMARY KEY AUTOINCREMENT,

    task_id INTEGER NOT NULL,

    payload_json TEXT NOT NULL
        CHECK(json_valid(payload_json)),

    captured_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,

    FOREIGN KEY(task_id)
        REFERENCES task_queue(id)
);

------------------------------------------------------
-- SCHEMA VERSIONS
------------------------------------------------------

CREATE TABLE IF NOT EXISTS schema_versions (
    id INTEGER PRIMARY KEY AUTOINCREMENT,

    endpoint TEXT NOT NULL,

    version_number INTEGER NOT NULL,

    schema_hash TEXT NOT NULL,

    created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,

    UNIQUE(endpoint, version_number),
    UNIQUE(endpoint, schema_hash),
    CHECK(version_number > 0)
);

------------------------------------------------------
-- SCHEMA SNAPSHOTS
------------------------------------------------------

CREATE TABLE IF NOT EXISTS schema_snapshots (
    id INTEGER PRIMARY KEY AUTOINCREMENT,

    schema_version_id INTEGER NOT NULL,

    schema_json TEXT NOT NULL
        CHECK(json_valid(schema_json)),

    created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,

    FOREIGN KEY(schema_version_id)
        REFERENCES schema_versions(id),

    UNIQUE(schema_version_id)
);

------------------------------------------------------
-- WORKER EVENTS
------------------------------------------------------

CREATE TABLE IF NOT EXISTS worker_events (
    id INTEGER PRIMARY KEY AUTOINCREMENT,

    task_id INTEGER,

    event_type TEXT NOT NULL
        CHECK(event_type IN (
            'TaskAssigned',
            'AssetCaptured',
            'SchemaVersionCreated',
            'TaskCompleted',
            'TaskFailed'
        )),

    details TEXT,

    created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,

    FOREIGN KEY(task_id)
        REFERENCES task_queue(id)
);

------------------------------------------------------
-- ÍNDICES
------------------------------------------------------

CREATE INDEX IF NOT EXISTS idx_task_status
ON task_queue(status);

CREATE INDEX IF NOT EXISTS idx_assets_task
ON assets(task_id);

CREATE INDEX IF NOT EXISTS idx_events_task
ON worker_events(task_id);

CREATE INDEX IF NOT EXISTS idx_schema_hash
ON schema_versions(schema_hash);

CREATE INDEX IF NOT EXISTS idx_status_name
ON status(name);

CREATE INDEX IF NOT EXISTS idx_schema_snapshots_version
ON schema_snapshots(schema_version_id);
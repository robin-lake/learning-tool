-- Append-only log of status changes (archive, suspend, retire, reword...), so the map can tell what
-- existed on past days. Never UPDATE or DELETE rows here.
CREATE TABLE status_events (
    id INTEGER PRIMARY KEY,
    entity TEXT NOT NULL CHECK (entity IN ('topic', 'question')),
    entity_id INTEGER NOT NULL,
    from_status TEXT NOT NULL,
    to_status TEXT NOT NULL,
    reason TEXT NOT NULL CHECK (reason IN ('archive', 'unarchive', 'suspend', 'unsuspend', 'retire', 'reword', 'reactivate')),
    at TEXT NOT NULL,
    created_at TEXT NOT NULL
);
CREATE INDEX status_events_entity ON status_events (entity, entity_id, at);

INSERT INTO settings (key, value) VALUES ('stale_after_days', '14'), ('stale_full_days', '60');

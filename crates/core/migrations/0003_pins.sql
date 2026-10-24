-- Pinned questions: asked first in the next study session (milestone 3 design, "Pins").
-- Rows are never deleted: unpinning sets unpinned_at, using a pin sets used_in_session_id.
CREATE TABLE pins (
    id INTEGER PRIMARY KEY,
    question_id INTEGER NOT NULL REFERENCES questions(id),
    pinned_at TEXT NOT NULL,
    unpinned_at TEXT,
    used_in_session_id INTEGER REFERENCES sessions(id),
    used_at TEXT,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);
CREATE UNIQUE INDEX pins_one_active ON pins (question_id)
    WHERE unpinned_at IS NULL AND used_in_session_id IS NULL;

ALTER TABLE session_items ADD COLUMN pin_id INTEGER REFERENCES pins(id);

INSERT INTO settings (key, value) VALUES ('web_port', '4747');

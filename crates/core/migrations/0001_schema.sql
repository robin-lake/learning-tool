-- Milestone 1 schema. See docs/superpowers/specs/2026-10-23-milestone-1-study-loop-design.md.
-- Timestamps are UTC RFC 3339 text; study days are 'YYYY-MM-DD' text.

CREATE TABLE projects (
    id INTEGER PRIMARY KEY,
    name TEXT NOT NULL,
    slug TEXT NOT NULL UNIQUE,
    description TEXT NOT NULL DEFAULT '',
    status TEXT NOT NULL DEFAULT 'active' CHECK (status IN ('active', 'archived')),
    desired_retention REAL NOT NULL DEFAULT 0.8,
    max_interval_days INTEGER,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE TABLE topics (
    id INTEGER PRIMARY KEY,
    parent_id INTEGER REFERENCES topics(id),
    name TEXT NOT NULL,
    slug TEXT NOT NULL,
    description TEXT NOT NULL DEFAULT '',
    scope TEXT NOT NULL DEFAULT 'main' CHECK (scope IN ('main', 'project')),
    project_id INTEGER REFERENCES projects(id),
    target_level INTEGER NOT NULL DEFAULT 1 CHECK (target_level BETWEEN 0 AND 3),
    status TEXT NOT NULL DEFAULT 'active' CHECK (status IN ('active', 'archived')),
    position INTEGER NOT NULL DEFAULT 0,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    CHECK (scope = 'main' OR project_id IS NOT NULL)
);
CREATE UNIQUE INDEX topics_sibling_slug ON topics (COALESCE(parent_id, 0), slug);

CREATE TABLE topic_links (
    id INTEGER PRIMARY KEY,
    from_topic_id INTEGER NOT NULL REFERENCES topics(id),
    to_topic_id INTEGER NOT NULL REFERENCES topics(id),
    kind TEXT NOT NULL CHECK (kind IN ('related', 'prerequisite', 'applies')),
    weight REAL NOT NULL CHECK (weight BETWEEN 0 AND 1),
    note TEXT NOT NULL DEFAULT '',
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    CHECK (from_topic_id <> to_topic_id),
    UNIQUE (from_topic_id, to_topic_id, kind)
);

CREATE TABLE guide_versions (
    id INTEGER PRIMARY KEY,
    content TEXT NOT NULL,
    change_summary TEXT NOT NULL DEFAULT '',
    parent_id INTEGER REFERENCES guide_versions(id),
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE TABLE sources (
    id INTEGER PRIMARY KEY,
    title TEXT NOT NULL,
    authors TEXT NOT NULL DEFAULT '',
    kind TEXT NOT NULL CHECK (kind IN ('pdf', 'article', 'book', 'docs', 'video', 'other')),
    url TEXT,
    file_path TEXT,
    text_path TEXT,
    summary TEXT NOT NULL DEFAULT '',
    status TEXT NOT NULL DEFAULT 'active' CHECK (status IN ('active', 'archived')),
    added_at TEXT NOT NULL,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE TABLE reference_topics (
    source_id INTEGER NOT NULL REFERENCES sources(id),
    topic_id INTEGER NOT NULL REFERENCES topics(id),
    PRIMARY KEY (source_id, topic_id)
);

CREATE TABLE reference_chunks (
    id INTEGER PRIMARY KEY,
    source_id INTEGER NOT NULL REFERENCES sources(id),
    locator TEXT NOT NULL,
    ordinal INTEGER NOT NULL,
    text TEXT NOT NULL,
    UNIQUE (source_id, ordinal)
);

CREATE VIRTUAL TABLE reference_chunks_fts USING fts5(
    text,
    content = 'reference_chunks',
    content_rowid = 'id',
    tokenize = 'porter unicode61'
);
CREATE TRIGGER reference_chunks_ai AFTER INSERT ON reference_chunks BEGIN
    INSERT INTO reference_chunks_fts (rowid, text) VALUES (new.id, new.text);
END;
CREATE TRIGGER reference_chunks_ad AFTER DELETE ON reference_chunks BEGIN
    INSERT INTO reference_chunks_fts (reference_chunks_fts, rowid, text) VALUES ('delete', old.id, old.text);
END;
CREATE TRIGGER reference_chunks_au AFTER UPDATE ON reference_chunks BEGIN
    INSERT INTO reference_chunks_fts (reference_chunks_fts, rowid, text) VALUES ('delete', old.id, old.text);
    INSERT INTO reference_chunks_fts (rowid, text) VALUES (new.id, new.text);
END;

CREATE TABLE questions (
    id INTEGER PRIMARY KEY,
    topic_id INTEGER NOT NULL REFERENCES topics(id),
    level INTEGER NOT NULL CHECK (level BETWEEN 0 AND 3),
    prompt TEXT NOT NULL,
    answer TEXT NOT NULL,
    rubric TEXT NOT NULL,              -- JSON [{id, point, weight, essential}]
    pass_threshold REAL NOT NULL DEFAULT 0.6 CHECK (pass_threshold > 0 AND pass_threshold <= 1),
    source_id INTEGER REFERENCES sources(id),
    source_locator TEXT,
    scope TEXT NOT NULL DEFAULT 'main' CHECK (scope IN ('main', 'project')),
    project_id INTEGER REFERENCES projects(id),
    status TEXT NOT NULL DEFAULT 'active' CHECK (status IN ('proposed', 'active', 'suspended', 'retired')),
    priority INTEGER NOT NULL DEFAULT 0,
    guide_version_id INTEGER REFERENCES guide_versions(id),
    revision_of INTEGER REFERENCES questions(id),
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    CHECK (scope = 'main' OR project_id IS NOT NULL)
);
CREATE INDEX questions_topic ON questions (topic_id);
CREATE INDEX questions_status_scope ON questions (status, scope);

CREATE TABLE question_topics (
    question_id INTEGER NOT NULL REFERENCES questions(id),
    topic_id INTEGER NOT NULL REFERENCES topics(id),
    PRIMARY KEY (question_id, topic_id)
);

CREATE TABLE question_links (
    id INTEGER PRIMARY KEY,
    from_question_id INTEGER NOT NULL REFERENCES questions(id),
    to_question_id INTEGER NOT NULL REFERENCES questions(id),
    kind TEXT NOT NULL CHECK (kind IN ('related', 'prerequisite', 'applies')),
    weight REAL NOT NULL CHECK (weight BETWEEN 0 AND 1),
    note TEXT NOT NULL DEFAULT '',
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    CHECK (from_question_id <> to_question_id),
    UNIQUE (from_question_id, to_question_id, kind)
);

CREATE TABLE sessions (
    id INTEGER PRIMARY KEY,
    kind TEXT NOT NULL CHECK (kind IN ('main', 'project')),
    project_id INTEGER REFERENCES projects(id),
    study_day TEXT NOT NULL,
    planned_count INTEGER NOT NULL DEFAULT 0,
    started_at TEXT NOT NULL,
    ended_at TEXT,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

-- Append-only review log: never UPDATE or DELETE rows in this table.
CREATE TABLE reviews (
    id INTEGER PRIMARY KEY,
    question_id INTEGER NOT NULL REFERENCES questions(id),
    session_id INTEGER NOT NULL REFERENCES sessions(id),
    reviewed_at TEXT NOT NULL,
    grade INTEGER NOT NULL CHECK (grade BETWEEN 1 AND 4),
    score REAL NOT NULL CHECK (score BETWEEN 0 AND 1),
    rubric_result TEXT NOT NULL,       -- JSON [{id, hit, note}]
    hints_used INTEGER NOT NULL DEFAULT 0,
    fluent INTEGER NOT NULL DEFAULT 0,
    user_answer TEXT NOT NULL,
    asked_prompt TEXT NOT NULL,
    feedback TEXT NOT NULL DEFAULT '',
    duration_ms INTEGER,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);
CREATE INDEX reviews_question ON reviews (question_id, reviewed_at);
CREATE INDEX reviews_session ON reviews (session_id);

-- Derived from reviews; rebuildable at any time.
CREATE TABLE question_state (
    question_id INTEGER PRIMARY KEY REFERENCES questions(id),
    stability REAL,
    difficulty REAL,
    due_on TEXT,
    last_reviewed_at TEXT,
    last_study_day TEXT,
    reps INTEGER NOT NULL DEFAULT 0,
    lapses INTEGER NOT NULL DEFAULT 0,
    state TEXT NOT NULL DEFAULT 'new' CHECK (state IN ('new', 'review', 'relearning'))
);
CREATE INDEX question_state_due ON question_state (due_on);

-- Persisted session queue so next_question survives a server restart.
CREATE TABLE session_items (
    id INTEGER PRIMARY KEY,
    session_id INTEGER NOT NULL REFERENCES sessions(id),
    ordinal INTEGER NOT NULL,
    question_id INTEGER NOT NULL REFERENCES questions(id),
    kind TEXT NOT NULL CHECK (kind IN ('review', 'new', 'requeue')),
    served_at TEXT,
    review_id INTEGER REFERENCES reviews(id),
    UNIQUE (session_id, ordinal)
);
CREATE INDEX session_items_session ON session_items (session_id, ordinal);

CREATE TABLE question_ratings (
    id INTEGER PRIMARY KEY,
    question_id INTEGER NOT NULL REFERENCES questions(id),
    review_id INTEGER REFERENCES reviews(id),
    rated_at TEXT NOT NULL,
    usefulness INTEGER NOT NULL CHECK (usefulness BETWEEN 1 AND 5),
    feedback TEXT,
    classification TEXT CHECK (classification IN ('question_quality', 'knowledge_value', 'positive')),
    processed_in_guide_version_id INTEGER REFERENCES guide_versions(id),
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE TABLE guide_examples (
    id INTEGER PRIMARY KEY,
    guide_version_id INTEGER NOT NULL REFERENCES guide_versions(id),
    question_id INTEGER NOT NULL REFERENCES questions(id),
    verdict TEXT NOT NULL CHECK (verdict IN ('good', 'bad')),
    level INTEGER NOT NULL CHECK (level BETWEEN 0 AND 3),
    reason TEXT NOT NULL DEFAULT '',
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE TABLE project_topics (
    project_id INTEGER NOT NULL REFERENCES projects(id),
    topic_id INTEGER NOT NULL REFERENCES topics(id),
    required_level INTEGER CHECK (required_level BETWEEN 0 AND 3),
    PRIMARY KEY (project_id, topic_id)
);

CREATE TABLE settings (
    key TEXT PRIMARY KEY,
    value TEXT NOT NULL
);

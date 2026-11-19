CREATE TABLE memories (
 id TEXT PRIMARY KEY NOT NULL, record_id TEXT NOT NULL, kind TEXT NOT NULL CHECK(kind IN ('decision','fact','experience')),
 body TEXT NOT NULL, target TEXT, created INTEGER NOT NULL,
 source TEXT, details TEXT NOT NULL CHECK(json_valid(details))
);
CREATE INDEX memory_history ON memories(record_id);
CREATE VIRTUAL TABLE evidence_index USING fts5(words,singles,subject,tokenize='unicode61 remove_diacritics 0');
CREATE TABLE notices (
 id TEXT PRIMARY KEY NOT NULL, kind TEXT NOT NULL, version TEXT NOT NULL, created INTEGER NOT NULL,
 call_id TEXT NOT NULL, opportunity TEXT, action TEXT, target TEXT, outcome TEXT
);
CREATE INDEX notices_by_version ON notices(version,kind);
CREATE INDEX notices_by_action ON notices(action,created);
CREATE TABLE calls (
 id TEXT PRIMARY KEY NOT NULL, kind TEXT NOT NULL, created INTEGER NOT NULL, elapsed_us INTEGER NOT NULL,
 bytes INTEGER NOT NULL, records TEXT NOT NULL, opportunity TEXT, action TEXT, target TEXT, query TEXT
);
CREATE INDEX calls_by_action ON calls(action,created);

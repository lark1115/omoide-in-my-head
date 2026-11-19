CREATE TABLE IF NOT EXISTS meta (
 singleton INTEGER PRIMARY KEY CHECK(singleton=1), revision INTEGER NOT NULL, usage_revision INTEGER NOT NULL,
 schema_version INTEGER NOT NULL CHECK(schema_version=2), store_id TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS revision_guard(ok INTEGER NOT NULL);
CREATE TABLE IF NOT EXISTS source_guard(ok INTEGER NOT NULL);
CREATE TABLE IF NOT EXISTS receipts(op TEXT PRIMARY KEY NOT NULL, digest TEXT NOT NULL, result TEXT NOT NULL);
CREATE TABLE IF NOT EXISTS utterances(id TEXT PRIMARY KEY NOT NULL, body TEXT NOT NULL, speaker TEXT NOT NULL, created INTEGER NOT NULL, target TEXT);
CREATE TABLE IF NOT EXISTS rules(
 rowid INTEGER PRIMARY KEY, id TEXT NOT NULL, version TEXT UNIQUE NOT NULL, state TEXT NOT NULL,
 body TEXT NOT NULL, scene TEXT NOT NULL, aliases TEXT NOT NULL, actions TEXT NOT NULL, target TEXT,
 purpose TEXT, exclusion TEXT, source TEXT NOT NULL REFERENCES utterances(id), created INTEGER NOT NULL, successor TEXT REFERENCES rules(version) DEFERRABLE INITIALLY DEFERRED,
 parent_id TEXT, pending_against TEXT REFERENCES rules(version)
);
CREATE TABLE IF NOT EXISTS action_links(action TEXT NOT NULL, version TEXT NOT NULL REFERENCES rules(version), PRIMARY KEY(action,version));
CREATE INDEX IF NOT EXISTS rules_state_target ON rules(state,target);
CREATE VIRTUAL TABLE IF NOT EXISTS rule_index USING fts5(words,singles,tokenize='unicode61 remove_diacritics 0');

CREATE UNIQUE INDEX IF NOT EXISTS one_current_version ON rules(id) WHERE state='current';
CREATE INDEX IF NOT EXISTS rule_history ON rules(id,rowid);
CREATE INDEX IF NOT EXISTS exception_parent ON rules(parent_id) WHERE parent_id IS NOT NULL;
CREATE INDEX IF NOT EXISTS pending_base ON rules(pending_against) WHERE state='pending';
CREATE TABLE IF NOT EXISTS extra_sources(version TEXT NOT NULL REFERENCES rules(version),
    source TEXT NOT NULL REFERENCES utterances(id), PRIMARY KEY(version,source));
CREATE TABLE IF NOT EXISTS facts(id TEXT PRIMARY KEY NOT NULL,body TEXT NOT NULL,source TEXT NOT NULL REFERENCES utterances(id),
    certainty TEXT NOT NULL CHECK(certainty IN ('unknown','reported')),target TEXT,created INTEGER NOT NULL);

CREATE TABLE IF NOT EXISTS meta (
 singleton INTEGER PRIMARY KEY CHECK(singleton=1), revision INTEGER NOT NULL, usage_revision INTEGER NOT NULL,
 schema_version INTEGER NOT NULL CHECK(schema_version=1), store_id TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS revision_guard(ok INTEGER NOT NULL);
CREATE TABLE IF NOT EXISTS source_guard(ok INTEGER NOT NULL);
CREATE TABLE IF NOT EXISTS receipts(op TEXT PRIMARY KEY NOT NULL, digest TEXT NOT NULL, result TEXT NOT NULL);
CREATE TABLE IF NOT EXISTS utterances(id TEXT PRIMARY KEY NOT NULL, body TEXT NOT NULL, speaker TEXT NOT NULL, created INTEGER NOT NULL, target TEXT);
CREATE TABLE IF NOT EXISTS rules(
 rowid INTEGER PRIMARY KEY, id TEXT UNIQUE NOT NULL, version TEXT UNIQUE NOT NULL, state TEXT NOT NULL,
 body TEXT NOT NULL, scene TEXT NOT NULL, aliases TEXT NOT NULL, actions TEXT NOT NULL, target TEXT,
 purpose TEXT, exclusion TEXT, source TEXT NOT NULL REFERENCES utterances(id), created INTEGER NOT NULL
);
CREATE TABLE IF NOT EXISTS action_links(action TEXT NOT NULL, rule_id TEXT NOT NULL REFERENCES rules(id), PRIMARY KEY(action,rule_id));
CREATE INDEX IF NOT EXISTS rules_state_target ON rules(state,target);
CREATE VIRTUAL TABLE IF NOT EXISTS rule_index USING fts5(words,singles,tokenize='unicode61 remove_diacritics 0');

ALTER TABLE meta RENAME TO meta_v3;
ALTER TABLE extra_sources RENAME TO extra_sources_v3;
ALTER TABLE action_links RENAME TO action_links_v3;
ALTER TABLE rules RENAME TO rules_v3;
DROP INDEX rules_state_target;
DROP INDEX one_current_version;
DROP INDEX rule_history;
DROP INDEX exception_parent;
DROP INDEX pending_base;

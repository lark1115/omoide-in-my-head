ALTER TABLE meta RENAME TO meta_v1;
ALTER TABLE action_links RENAME TO action_links_v1;
ALTER TABLE rules RENAME TO rules_v1;
DROP INDEX rules_state_target;

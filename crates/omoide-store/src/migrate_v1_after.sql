INSERT INTO meta SELECT singleton, revision+1, usage_revision, 2, store_id FROM meta_v1;
INSERT INTO rules(rowid,id,version,state,body,scene,aliases,actions,target,purpose,exclusion,source,created)
    SELECT rowid,id,version,state,body,scene,aliases,actions,target,purpose,exclusion,source,created FROM rules_v1;
INSERT INTO action_links SELECT a.action,r.version FROM action_links_v1 a JOIN rules_v1 r ON r.id=a.rule_id;
DROP TABLE action_links_v1;
DROP TABLE rules_v1;
DROP TABLE meta_v1;

INSERT INTO meta SELECT singleton,revision+1,usage_revision,4,store_id FROM meta_v3;
INSERT INTO rules(rowid,id,version,state,body,scene,aliases,actions,target,purpose,exclusion,source,created,successor,parent_id,pending_against)
    SELECT rowid,id,version,state,body,scene,aliases,actions,target,purpose,exclusion,source,created,successor,parent_id,pending_against FROM rules_v3;
INSERT INTO extra_sources SELECT * FROM extra_sources_v3;
INSERT INTO action_links SELECT * FROM action_links_v3;
DROP TABLE extra_sources_v3;
DROP TABLE action_links_v3;
DROP TABLE rules_v3;
DROP TABLE meta_v3;

use crate::{
    args::{Rule, SkillCommand},
    db::*,
    relations::{self, Plan},
    write::{create_rule, nonempty},
};
use omoide_store::{
    Error, Rows, Statement, Store, Value,
    lookup::{RULE, procedure_source},
};
use serde_json::{Value as Json, json};
use std::collections::{BTreeMap, BTreeSet};

pub fn repeated(db: &mut impl Store, min: u32) -> Result<Json> {
    let rows = db.read_batch(&[q(
        "SELECT id,body,target FROM memories WHERE kind='experience' ORDER BY created,id",
        vec![],
    )])?;
    let mut groups = BTreeMap::new();
    for r in &rows[0] {
        let key = omoide_core::tokens(text(&r[1])?).words;
        if key.is_empty() {
            continue;
        }
        let target = match &r[2] {
            Value::Text(v) => Some(v.as_str()),
            _ => None,
        };
        groups
            .entry((target, key))
            .or_insert_with(Vec::new)
            .push(text(&r[0])?);
    }
    let groups:Vec<_>=groups.into_iter().filter(|(_,v)|v.len()>=min as usize).map(|((target,_),sources)|json!({"target":target,"count":sources.len(),"sources":sources})).collect();
    Ok(json!({"groups":groups}))
}
pub fn queries(cmd: &SkillCommand) -> Vec<Statement> {
    match cmd {
        SkillCommand::Propose { sources, .. } => vec![q(
            "SELECT id FROM memories WHERE kind='experience' AND id IN (SELECT value FROM json_each(?))",
            vec![s(&json!(sources).to_string())],
        )],
        SkillCommand::Confirm {
            version, against, ..
        } => {
            let source = procedure_source("u");
            let mut queries=vec![
                q(format!("SELECT {source},{RULE} FROM rules r JOIN origins u ON u.id=r.source WHERE r.version=? AND r.kind='skill' AND r.state='candidate'"),vec![s(version)]),
                q(format!("SELECT {RULE},{source} FROM rules r JOIN origins u ON u.id=r.source WHERE r.kind='rule' AND r.state='current'
                   AND (r.version IN (SELECT version FROM action_links WHERE action IN (SELECT action FROM action_links WHERE version=?1))
                   OR r.rowid IN (SELECT rowid FROM rule_index WHERE rule_index MATCH coalesce((SELECT nullif(near_expression,'') FROM rules WHERE version=?1),'words : \"\"'))) ORDER BY r.version"),vec![s(version)])];
            if let Some(v) = against {
                queries.extend([relations::conflicts(v), relations::adjacent_scopes(v)]);
            }
            queries.push(omoide_store::near_counts::candidate(version));
            queries
        }
    }
}
fn strings(v: &Json) -> Vec<String> {
    v.as_array()
        .into_iter()
        .flatten()
        .filter_map(Json::as_str)
        .map(str::to_owned)
        .collect()
}
pub fn plan(
    cmd: &SkillCommand,
    sets: &[Rows],
    op: &str,
    id: &str,
    version: &str,
    time: i64,
) -> Result<Plan> {
    match cmd {
        SkillCommand::Propose {
            sources,
            text,
            scene,
            actions,
            target,
            reference,
        } => {
            nonempty(text)?;
            nonempty(scene)?;
            if sources.len() != sets[2].len() || sources.is_empty() {
                return Err(Error::Input(
                    "unique existing source experiences required".into(),
                ));
            }
            if actions.iter().any(|a| a.trim().is_empty())
                || actions.iter().collect::<BTreeSet<_>>().len() != actions.len()
            {
                return Err(Error::Input("invalid skill actions".into()));
            }
            let r = Rule {
                source: sources[0].clone(),
                text: text.clone(),
                scene: scene.clone(),
                aliases: vec![],
                actions: actions.clone(),
                target: target.clone(),
                purpose: None,
                exclude: None,
                pinned: false,
                relation: None,
                against: None,
                near_digest: None,
            };
            let mut batch = create_rule(&r, id, version, time, "candidate", None, None);
            let expression =
                omoide_store::lookup::expression(&omoide_core::document(text, scene, &[]));
            batch.push(q(
                "UPDATE rules SET kind='skill',reference=?,near_expression=? WHERE version=?",
                vec![opt(reference), s(&expression), s(version)],
            ));
            for source in sources.iter().skip(1) {
                batch.push(q(
                    "INSERT INTO extra_sources VALUES(?,?)",
                    vec![s(version), s(source)],
                ));
            }
            Ok(Plan::Write(
                batch,
                json!({"state":"candidate","kind":"skill","id":id,"version_id":version,"operation_id":op}),
            ))
        }
        SkillCommand::Confirm {
            version: draft,
            relation,
            against,
            near_digest,
        } => {
            let row = sets[2]
                .first()
                .ok_or_else(|| Error::Input("skill candidate not found".into()))?;
            let data = parse(&row[1])?;
            let rule = Rule {
                source: data["source_id"].as_str().unwrap().into(),
                text: data["text"].as_str().unwrap().into(),
                scene: data["scene"].as_str().unwrap().into(),
                actions: strings(&data["actions"]),
                aliases: strings(&data["aliases"]),
                target: data["target"].as_str().map(str::to_owned),
                purpose: None,
                exclude: None,
                pinned: false,
                relation: relation.clone(),
                against: against.clone(),
                near_digest: near_digest.clone(),
            };
            let plan =
                relations::plan(&rule, sets, op, data["id"].as_str().unwrap(), version, time)?;
            let Plan::Write(mut batch, mut result) = plan else {
                return Ok(plan);
            };
            let destination = result["version_id"].as_str().unwrap().to_owned();
            if result["state"] != "same" {
                batch.push(q("UPDATE rules SET kind='skill',reference=?,near_expression=(SELECT near_expression FROM rules WHERE version=?) WHERE version=?",vec![opt(&data["reference"].as_str().map(str::to_owned)),s(draft),s(version)]));
                result["kind"] = json!("skill");
                if let Some(pairs) = result["unresolved_conflicts"].as_array_mut() {
                    pairs[0]["pending"]["record"]["kind"] = json!("skill");
                }
            }
            batch.push(q("INSERT OR IGNORE INTO extra_sources(version,source) SELECT ?1,source FROM extra_sources WHERE version=?2 AND source<>(SELECT source FROM rules WHERE version=?1)",vec![s(&destination),s(draft)]));
            batch.push(q(
                "UPDATE rules SET state='promoted',successor=? WHERE version=?",
                vec![s(&destination), s(draft)],
            ));
            Ok(Plan::Write(batch, result))
        }
    }
}

use crate::{args::Rule, db::*, search::near, write::create_rule};
use omoide_core::{Outcome, Relation, document, registration};
use omoide_store::{Error, Rows, Statement};
use serde_json::{Value as Json, json};

pub enum Plan {
    Reply(Json),
    Write(Vec<Statement>, Json),
}

fn relation(name: Option<&str>) -> Result<Option<Relation>> {
    Ok(match name {
        None => None,
        Some("same") => Some(Relation::Same),
        Some("restated") => Some(Relation::Revision),
        Some("exception") => Some(Relation::Exception),
        Some("unrelated") => Some(Relation::Unrelated),
        Some("unsure") => Some(Relation::Unclear),
        Some(_) => return Err(Error::Input("unknown relation".into())),
    })
}

fn supersede(old: &str, new: &str) -> Statement {
    q(
        "UPDATE rules SET state='superseded',successor=? WHERE version=?",
        vec![s(new), s(old)],
    )
}

pub fn conflicts(selected: &str) -> Statement {
    q(
        "SELECT pending_against,version FROM rules WHERE state='pending' AND pending_against IN (?1,(SELECT pending_against FROM rules WHERE version=?1))",
        vec![s(selected)],
    )
}

/// Adjacent scopes constrain a replacement even when its parent or children
/// were not themselves lexical neighbors of the proposal.
pub fn adjacent_scopes(version: &str) -> Statement {
    q("SELECT 'parent',p.target FROM rules r JOIN rules p ON p.id=r.parent_id AND p.state='current' WHERE r.version=?1
       UNION ALL SELECT 'child',c.target FROM rules r JOIN rules c ON c.parent_id=r.id AND c.state='current' WHERE r.version=?1",
      vec![s(version)])
}
fn check_scope(rows: &Rows, target: Option<&str>) -> Result<()> {
    use omoide_store::Value;
    for row in rows {
        let bound = match &row[1] {
            Value::Null => None,
            value => Some(text(value)?),
        };
        let outside = match text(&row[0])? {
            "parent" => target.is_none() || (bound.is_some() && bound != target),
            _ => target.is_some() && bound != target,
        };
        if outside {
            return Err(Error::Input(
                "proposed target conflicts with a linked exception scope".into(),
            ));
        }
    }
    Ok(())
}
pub fn plan(
    rule: &Rule,
    sets: &[Rows],
    op: &str,
    new_id: &str,
    version: &str,
    time: i64,
) -> Result<Plan> {
    use Outcome::*;
    let utterance = required_record(&sets[2])?;
    let terms = document(&rule.text, &rule.scene, &rule.aliases);
    let mut neighbors = Vec::new();
    for row in &sets[3] {
        let record = parse(&row[0])?;
        if near(&record, &terms, &rule.actions) {
            neighbors.push(json!({"record":record,"source":parse(&row[1])?}));
        }
    }
    let versions: Vec<_> = neighbors
        .iter()
        .map(|n| &n["record"]["version_id"])
        .collect();
    let hash = digest(json!(versions).to_string());
    let outcome = registration(!neighbors.is_empty(), relation(rule.relation.as_deref())?);
    if outcome == NeedRelation
        || rule
            .near_digest
            .as_ref()
            .is_some_and(|previous| previous != &hash)
    {
        return Ok(Plan::Reply(
            json!({"state":"near_rules","operation_id":op,"near_digest":hash,"near_rules":neighbors}),
        ));
    }
    let selected = match rule.against.as_deref() {
        Some(v) => Some(
            neighbors
                .iter()
                .find(|n| n["record"]["version_id"] == v)
                .ok_or_else(|| Error::Input("selected version is not a current neighbor".into()))?,
        ),
        None => None,
    };
    let mut changes = Vec::new();
    let mut id = new_id;
    let mut parent = None;
    let mut base = None;
    let mut state = "current";
    let mut result_state = "activated";
    let mut evidence = None;
    if let Some(selected) = selected {
        let record = &selected["record"];
        let old = record["version_id"].as_str().unwrap();
        let old_id = record["id"].as_str().unwrap();
        match outcome {
            AddSource => {
                changes.push(q("INSERT INTO extra_sources(version,source) SELECT ?1,?2 WHERE ?2<>(SELECT source FROM rules WHERE version=?1) ON CONFLICT(version,source) DO NOTHING",vec![s(old),s(&rule.source)]));
                return Ok(Plan::Write(
                    changes,
                    json!({"state":"same","kind":"rule","id":old_id,"version_id":old,"operation_id":op}),
                ));
            }
            Replace | Pending => {
                check_scope(&sets[5], rule.target.as_deref())?;
                if outcome == Pending
                    && record["target"]
                        .as_str()
                        .zip(rule.target.as_deref())
                        .is_some_and(|(a, b)| a != b)
                {
                    return Err(Error::Input(
                        "disjoint targets cannot form an applicable conflict".into(),
                    ));
                }
                id = old_id;
                parent = record["parent_id"].as_str();
                if parent.is_some() && rule.target.as_deref().is_none_or(|t| t.trim().is_empty()) {
                    return Err(Error::Input("exception requires a target".into()));
                }
                if outcome == Replace {
                    if sets[4].iter().any(|r| r[0] == s(old)) {
                        return Err(Error::Input(
                            "resolve pending conflicts before restating this version".into(),
                        ));
                    }
                    changes.push(supersede(old, version));
                    result_state = "replaced";
                } else {
                    base = Some(old);
                    state = "pending";
                    result_state = "pending";
                    evidence = Some(selected);
                }
            }
            ActivateException => {
                let target = rule
                    .target
                    .as_deref()
                    .filter(|s| !s.trim().is_empty())
                    .ok_or_else(|| Error::Input("exception requires a target".into()))?;
                if record["target"].as_str().is_some_and(|t| t != target) {
                    return Err(Error::Input(
                        "exception target is outside the parent scope".into(),
                    ));
                }
                parent = Some(old_id);
            }
            Activate => (),
            _ => return Err(Error::Input("invalid relation".into())),
        }
    } else if outcome != Activate {
        return Err(Error::Input(
            "relation requires a selected current version".into(),
        ));
    }
    changes.extend(create_rule(rule, id, version, time, state, parent, base));
    let mut result =
        json!({"state":result_state,"kind":"rule","id":id,"version_id":version,"operation_id":op});
    if let Some(current) = evidence {
        let pending = json!({"id":id,"version_id":version,"kind":"rule","state":"pending","text":rule.text,"scene":rule.scene,"target":rule.target,"actions":rule.actions,"purpose":rule.purpose,"exclusion":rule.exclude,"source_id":rule.source});
        result["unresolved_conflicts"] =
            json!([{"current":current,"pending":{"record":pending,"source":utterance}}]);
    }
    Ok(Plan::Write(changes, result))
}

pub fn resolve(
    rows: &Rows,
    open: &Rows,
    scopes: &Rows,
    pending: &str,
    keep: &str,
    op: &str,
) -> Result<(Vec<Statement>, Json)> {
    let r = rows
        .first()
        .ok_or_else(|| Error::Input("pending version or current opponent not found".into()))?;
    let id = text(&r[0])?;
    let old = text(&r[1])?;
    let (changes, current) = if keep == "pending" {
        let target = match &r[2] {
            omoide_store::Value::Null => None,
            v => Some(text(v)?),
        };
        check_scope(scopes, target)?;
        if open.iter().any(|r| r[0] == s(old) && r[1] != s(pending)) {
            return Err(Error::Input(
                "resolve other pending proposals against this version first".into(),
            ));
        }
        (
            vec![
                supersede(old, pending),
                q(
                    "UPDATE rules SET state='current' WHERE version=?",
                    vec![s(pending)],
                ),
            ],
            pending,
        )
    } else {
        (
            vec![q(
                "UPDATE rules SET state='rejected' WHERE version=?",
                vec![s(pending)],
            )],
            old,
        )
    };
    Ok((
        changes,
        json!({"state":"resolved","kind":"rule","id":id,"version_id":current,"operation_id":op}),
    ))
}

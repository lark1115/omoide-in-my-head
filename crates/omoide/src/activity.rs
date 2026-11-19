use crate::{
    args::{Command, Notice},
    db::*,
};
use omoide_store::{Error, Rows, Statement, Store, Value};
use serde_json::{Value as Json, json};

pub fn notice(
    (kind, n, outcome): (&str, &Notice, Option<&str>),
    records: &Rows,
    prior: Option<&Rows>,
    id: &str,
    at: i64,
    opportunity: Option<&str>,
    call_id: &str,
) -> Result<(Vec<Statement>, Json)> {
    let record = records
        .first()
        .ok_or_else(|| Error::Input("unknown record version".into()))?;
    let actions = parse(&record[1])?;
    let action = n.action.as_deref().or_else(|| {
        actions
            .as_array()
            .filter(|a| a.len() == 1)?
            .first()?
            .as_str()
    });
    if kind == "acted" && action.is_none() {
        return Err(Error::Input("acted requires an action name".into()));
    }
    if let Some(action) = &n.action {
        crate::write::nonempty(action)?;
        if actions
            .as_array()
            .is_some_and(|names| !names.is_empty() && !names.iter().any(|a| a == action))
        {
            return Err(Error::Input(
                "action does not belong to this rule version".into(),
            ));
        }
    }
    let original =
        match prior {
            Some(rows) => Some(rows.first().ok_or_else(|| {
                Error::Input("recall did not return the referenced version".into())
            })?),
            None => None,
        };
    let inherited = original.and_then(|r| match &r[0] {
        Value::Text(v) => Some(v.as_str()),
        _ => None,
    });
    if opportunity.zip(inherited).is_some_and(|(a, b)| a != b) {
        return Err(Error::Input(
            "opportunity differs from original call".into(),
        ));
    }
    let opportunity = opportunity.or(inherited);
    let call = n.from_call.as_deref().unwrap_or(call_id);
    let stmt = q(
        "INSERT INTO notices(id,kind,version,created,call_id,opportunity,action,target,outcome)
        VALUES(?,?,?,?,?,?,?,?,?)",
        vec![
            s(id),
            s(kind),
            s(&n.version),
            Value::Integer(at),
            s(call),
            opportunity.map_or(Value::Null, s),
            action.map_or(Value::Null, s),
            record[0].clone(),
            outcome.map_or(Value::Null, s),
        ],
    );
    Ok((
        vec![stmt],
        json!({"state":"recorded","notice_id":id,"operation_id":id,"kind":kind,"version_id":n.version}),
    ))
}

pub fn stats(db: &mut impl Store, threshold: u32) -> Result<Json> {
    let sql="WITH opportunities AS (
        SELECT opportunity,action,target,created FROM calls WHERE opportunity IS NOT NULL AND action IS NOT NULL
        UNION ALL SELECT opportunity,action,target,created FROM notices WHERE kind='acted' AND opportunity IS NOT NULL
    ), lifetimes AS (SELECT id,min(created) AS began FROM rules GROUP BY id)
    SELECT r.id,r.version,
     (SELECT count(DISTINCT coalesce('op:'||n.opportunity,'notice:'||n.id)) FROM notices n JOIN rules v ON v.version=n.version
      WHERE v.id=r.id AND n.kind='corrected' AND n.created>=l.began),
     (SELECT count(DISTINCT o.opportunity) FROM opportunities o
      WHERE o.created>=l.began AND (r.target IS NULL OR o.target=r.target)
        AND (r.exclusion IS NULL OR r.exclusion IS NOT o.target)
        AND o.action IN (SELECT action FROM action_links WHERE version=r.version))
    FROM rules r JOIN lifetimes l ON l.id=r.id WHERE r.state='current' ORDER BY r.id";
    let sets = db.read_batch(&[q(sql, vec![])])?;
    let mut rules = Vec::new();
    let mut candidates = Vec::new();
    for row in &sets[0] {
        let id = text(&row[0])?;
        let corrections = int(&row[2])?;
        let opportunities = int(&row[3])?;
        if corrections >= i64::from(threshold) {
            candidates.push(id);
        }
        rules.push(json!({"id":id,"version_id":text(&row[1])?,"corrections":corrections,"opportunities":opportunities,
            "measurement":if opportunities==0{"not_measured"}else{"measured"}}));
    }
    Ok(json!({"rules":rules,"check_candidates":candidates,"correction_threshold":threshold}))
}

fn returned_records(value: &Json, out: &mut std::collections::BTreeMap<String, Json>) {
    match value {
        Json::Array(items) => {
            for item in items {
                returned_records(item, out);
            }
        }
        Json::Object(obj) => {
            if let Some(id) = obj.get("id").and_then(Json::as_str) {
                let version = obj.get("version_id").and_then(Json::as_str).unwrap_or(id);
                out.insert(version.into(), json!({"id":id,"version_id":version}));
            }
            for child in obj.values() {
                returned_records(child, out);
            }
        }
        _ => (),
    }
}

/// Record successful registrations and recalls. Logging is observational and never changes a revision.
pub fn log(
    db: &mut impl Store,
    cmd: &Command,
    id: &str,
    opportunity: &Option<String>,
    created: i64,
    elapsed: i64,
    reply: &Json,
) -> Result<()> {
    let mut returned = std::collections::BTreeMap::new();
    returned_records(reply, &mut returned);
    let records = json!(returned.into_values().collect::<Vec<_>>());
    let (kind, action, target, query) = match cmd {
        Command::Recall(q) => (
            "recall",
            opt(&q.action),
            opt(&q.target),
            if q.log_query {
                opt(&q.scene)
            } else {
                Value::Null
            },
        ),
        Command::Show { .. } => ("show", Value::Null, Value::Null, Value::Null),
        Command::Stats { .. } => ("stats", Value::Null, Value::Null, Value::Null),
        cmd => (
            cmd.notice().map_or("register", |(kind, _, _)| kind),
            Value::Null,
            Value::Null,
            Value::Null,
        ),
    };
    let stmt = q(
        "INSERT INTO calls(id,kind,created,elapsed_us,bytes,records,opportunity,action,target,query) VALUES(?,?,?,?,?,?,?,?,?,?)",
        vec![
            s(id),
            s(kind),
            Value::Integer(created),
            Value::Integer(elapsed),
            Value::Integer(reply.to_string().len() as i64 + 1),
            s(&records.to_string()),
            opt(opportunity),
            action,
            target,
            query,
        ],
    );
    db.exec_batch(&[stmt])
}

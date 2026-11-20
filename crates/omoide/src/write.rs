use crate::{
    args::{Command, FactCommand, Rule, RuleCommand},
    db::*,
    relations::{self, Plan},
};
use omoide_core::document;
use omoide_store::lookup::{index_rule, near_query};
use omoide_store::{Error, Statement, Store, Value, guarded, receipt, revision, save_receipt};
use serde_json::{Value as Json, json};
use std::{
    thread,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};
use uuid::Uuid;
fn id() -> String {
    Uuid::new_v4().to_string()
}
pub fn now() -> Result<i64> {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .map_err(|e| Error::Unknown(e.to_string()))
}
pub fn nonempty(text: &str) -> Result<()> {
    if text.trim().is_empty() {
        Err(Error::Input("required value is blank".into()))
    } else {
        Ok(())
    }
}
pub(crate) fn create_rule(
    rule: &Rule,
    rid: &str,
    vid: &str,
    time: i64,
    state: &str,
    parent: Option<&str>,
    base: Option<&str>,
) -> Vec<Statement> {
    let t = document(&rule.text, &rule.scene, &rule.aliases);
    let joined = |set: &std::collections::BTreeSet<String>| {
        set.iter().map(String::as_str).collect::<Vec<_>>().join(" ")
    };
    let mut changes = vec![
        q(
            "INSERT INTO source_guard VALUES(CASE WHEN EXISTS(SELECT 1 FROM origins WHERE id=?) THEN 1 ELSE NULL END)",
            vec![s(&rule.source)],
        ),
        q("DELETE FROM source_guard", vec![]),
        q(
            "INSERT INTO rules(id,version,state,body,scene,aliases,actions,target,purpose,exclusion,source,created,parent_id,pending_against,pinned) VALUES(?,?,?,?,?,?,?,?,?,?,?,?,?,?,?)",
            vec![
                s(rid),
                s(vid),
                s(state),
                s(&rule.text),
                s(&rule.scene),
                s(&json!(rule.aliases).to_string()),
                s(&json!(rule.actions).to_string()),
                opt(&rule.target),
                opt(&rule.purpose),
                opt(&rule.exclude),
                s(&rule.source),
                Value::Integer(time),
                parent.map_or(Value::Null, s),
                base.map_or(Value::Null, s),
                Value::Integer(i64::from(rule.pinned)),
            ],
        ),
        index_rule(vid, &joined(&t.words), &joined(&t.singles)),
    ];
    for action in &rule.actions {
        changes.push(q(
            "INSERT INTO action_links VALUES(?,?)",
            vec![s(action), s(vid)],
        ));
    }
    changes
}
fn validate_size(input_bytes: usize, id_bytes: usize) -> Result<()> {
    if input_bytes > 1_900_000 || id_bytes > 4096 {
        return Err(Error::Input("input exceeds storage limits".into()));
    }
    Ok(())
}
fn retry_revision(elapsed: Duration) -> Result<()> {
    if elapsed >= Duration::from_secs(5) {
        return Err(Error::Busy);
    }
    thread::sleep(Duration::from_millis(2));
    Ok(())
}
pub fn apply(
    db: &mut impl Store,
    command: &Command,
    op: &str,
    opportunity: Option<&str>,
    call_id: &str,
) -> Result<Json> {
    nonempty(op)?;
    let input = serde_json::to_vec(command).map_err(|e| Error::Input(e.to_string()))?;
    validate_size(input.len(), op.len())?;
    let notice = command.notice();
    let hash = if let Some((kind, n, outcome)) = notice {
        let mut fields = json!([kind, n.version, n.action, n.from_call, outcome, opportunity]);
        if let Command::Outcome { demote_after, .. } = command
            && *demote_after > 0
        {
            fields.as_array_mut().unwrap().push(json!(demote_after));
        }
        digest(fields.to_string())
    } else {
        digest(input)
    };
    let mut queries = vec![revision(), receipt(op)];
    if let Some((_, n, _)) = notice {
        queries[0] = q("SELECT usage_revision FROM meta WHERE singleton=1", vec![]);
        queries.push(q("SELECT target,actions,kind,state,failure_streak FROM rules WHERE version=?1 UNION ALL SELECT target,'[]',kind,'current',0 FROM memories WHERE id=?1",vec![s(&n.version)]));
        if let Some(from) = &n.from_call {
            queries.push(q(
                "SELECT opportunity FROM calls WHERE id=? AND kind='recall' AND EXISTS(
                SELECT 1 FROM json_each(calls.records) WHERE json_extract(value,'$.version_id')=?)",
                vec![s(from), s(&n.version)],
            ));
        }
    }
    if matches!(command, Command::Outcome { .. }) {
        queries.push(revision());
    }
    match command {
        Command::Skill { command } => queries.extend(crate::skills::queries(command)),
        Command::Init => queries.push(q("SELECT store_id FROM meta WHERE singleton=1", vec![])),
        Command::Say { text, speaker, .. } => {
            nonempty(text)?;
            nonempty(speaker)?;
        }
        Command::Rule {
            command: RuleCommand::Add(rule),
        } => {
            nonempty(&rule.scene)?;
            nonempty(&rule.text)?;
            nonempty(&rule.source)?;
            let mut unique = std::collections::BTreeSet::new();
            for action in &rule.actions {
                nonempty(action)?;
                if !unique.insert(action) {
                    return Err(Error::Input("duplicate action".into()));
                }
            }
            let terms = document(&rule.text, &rule.scene, &rule.aliases);
            queries.extend([
                source(&rule.source),
                near_query(&terms, &json!(rule.actions).to_string()),
            ]);
            if let Some(version) = &rule.against {
                queries.push(relations::conflicts(version));
                queries.push(relations::adjacent_scopes(version));
            }
            queries.push(omoide_store::near_counts::proposal(&terms));
        }
        Command::Fact {
            command: FactCommand::Add { source: id, .. },
        } => queries.push(omoide_store::evidence::origin(id)),
        Command::Decide(d) => {
            queries.push(source(&d.source));
            if let Some(old) = &d.revises {
                queries.push(q("SELECT record_id FROM memories WHERE id=? AND kind='decision' AND json_extract(details,'$.decision')!='revised'",vec![s(old)]));
            }
        }
        Command::Event { .. }
        | Command::Used(_)
        | Command::Acted(_)
        | Command::Outcome { .. }
        | Command::Corrected(_) => (),
        Command::Resolve { pending, .. } => {
            queries.push(q("SELECT p.id,c.version,p.target,c.kind,p.kind FROM rules p JOIN rules c ON c.version=p.pending_against WHERE p.version=? AND p.state='pending' AND c.state='current'",vec![s(pending)]));
            queries.push(relations::conflicts(pending));
            queries.push(relations::adjacent_scopes(pending));
        }
        _ => unreachable!(),
    }
    let start = Instant::now();
    let time = now()?;
    let rid = id();
    let vid = id();
    loop {
        let sets = db.read_batch(&queries)?;
        let rev = number(&sets[0])?;
        if let Some(row) = sets[1].first() {
            if text(&row[0])? != hash {
                return Err(Error::Input(
                    "operation id reused with different input".into(),
                ));
            }
            return parse(&row[1]);
        }
        let (mut changes, result) = match command {
            Command::Skill { command } => {
                match crate::skills::plan(command, &sets, op, &rid, &vid, time)? {
                    Plan::Reply(result) => return Ok(result),
                    Plan::Write(batch, result) => (batch, result),
                }
            }
            Command::Init => {
                let store_id = text(
                    &sets[2]
                        .first()
                        .ok_or_else(|| Error::Unknown("missing store id".into()))?[0],
                )?;
                (
                    vec![],
                    json!({"state":"ready","store_id":store_id,"operation_id":op}),
                )
            }
            Command::Say {
                text,
                speaker,
                target,
                at,
            } => (
                vec![q(
                    "INSERT INTO utterances(id,body,speaker,created,target) VALUES(?,?,?,?,?)",
                    vec![
                        s(&rid),
                        s(text),
                        s(speaker),
                        Value::Integer(at.unwrap_or(time)),
                        opt(target),
                    ],
                )],
                json!({"state":"stored","kind":"utterance","id":rid,"operation_id":op}),
            ),
            Command::Rule {
                command: RuleCommand::Add(rule),
            } => match relations::plan(rule, &sets, op, &rid, &vid, time)? {
                Plan::Reply(result) => return Ok(result),
                Plan::Write(batch, result) => (batch, result),
            },
            Command::Resolve { pending, keep } => {
                relations::resolve(&sets[2], &sets[3], &sets[4], pending, keep, op)?
            }
            Command::Fact { .. } | Command::Decide(_) | Command::Event { .. } => {
                crate::memory::insert(command, (&rid, &vid), time, &sets[2..], op)?
            }
            Command::Used(_)
            | Command::Acted(_)
            | Command::Corrected(_)
            | Command::Outcome { .. } => crate::activity::notice(
                notice.unwrap(),
                &sets[2],
                notice
                    .unwrap()
                    .1
                    .from_call
                    .as_ref()
                    .and_then(|_| sets.get(3)),
                op,
                time,
                opportunity,
                call_id,
            )?,
            _ => unreachable!(),
        };
        let mut demote = false;
        if let Command::Outcome {
            result: outcome,
            demote_after,
            ..
        } = command
        {
            let record = &sets[2][0];
            if text(&record[2])? == "skill" {
                let streak = if outcome == "failure" {
                    int(&record[4])? + 1
                } else {
                    0
                };
                demote = *demote_after > 0
                    && streak >= i64::from(*demote_after)
                    && text(&record[3])? == "current";
                changes.push(q("UPDATE rules SET failure_streak=?,state=CASE WHEN ? THEN 'candidate' ELSE state END WHERE version=?",
                    vec![Value::Integer(streak),Value::Integer(i64::from(demote)),s(&notice.unwrap().1.version)]));
            }
        }
        changes.push(save_receipt(op, &hash, &result.to_string()));
        let batch = if notice.is_some() {
            omoide_store::guarded_usage(rev, changes)
        } else {
            guarded(rev, changes)
        };
        let batch = if demote {
            guarded(number(sets.last().unwrap())?, batch)
        } else {
            batch
        };
        match db.exec_batch(&batch) {
            Ok(()) => return Ok(result),
            Err(Error::Revision) => retry_revision(start.elapsed())?,
            Err(e) => return Err(e),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::args::Cli;
    use clap::Parser;
    use omoide_store::{Rows, Sqlite};

    // [REQ-STORE-1]
    #[test]
    fn command_size_and_retry_deadline_boundaries() {
        assert!(validate_size(1_900_000, 4096).is_ok());
        assert!(validate_size(1_900_001, 4096).is_err());
        assert!(validate_size(1, 4097).is_err());
        assert!(retry_revision(Duration::from_millis(4999)).is_ok());
        assert!(matches!(
            retry_revision(Duration::from_secs(5)),
            Err(Error::Busy)
        ));
        assert!(matches!(
            retry_revision(Duration::from_secs(6)),
            Err(Error::Busy)
        ));
    }

    // [REQ-STORE-1]
    #[test]
    fn racing_registration_rechecks_neighbors_before_activation() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("race.db");
        let mut db = Sqlite::open(&path, true).unwrap();
        db.initialize("fixture", &[]).unwrap();
        let source =
            Cli::parse_from(["omoide", "say", "--text", "A synthetic instruction"]).command;
        let source_id = apply(&mut db, &source, "source", None, "source-call").unwrap()["id"]
            .as_str()
            .unwrap()
            .to_owned();
        let rule = Cli::parse_from([
            "omoide",
            "rule",
            "add",
            "--source",
            &source_id,
            "--text",
            "Check the press",
            "--scene",
            "press setup",
            "--action",
            "start",
        ])
        .command;
        struct Race<'a> {
            reader: Sqlite,
            writer: Sqlite,
            rule: &'a Command,
            injected: bool,
        }
        impl Store for Race<'_> {
            fn read_batch(&mut self, queries: &[Statement]) -> Result<Vec<Rows>> {
                let snapshot = self.reader.read_batch(queries)?;
                // Each registration decision must read the receipt, source, neighbors and revision together.
                assert_eq!(queries.len(), 5);
                if !self.injected {
                    self.injected = true;
                    assert_eq!(
                        apply(&mut self.writer, self.rule, "winner", None, "winner-call")?["state"],
                        "activated"
                    );
                }
                Ok(snapshot)
            }
            fn exec_batch(&mut self, statements: &[Statement]) -> Result<()> {
                self.reader.exec_batch(statements)
            }
        }
        let mut race = Race {
            reader: db,
            writer: Sqlite::open(&path, false).unwrap(),
            rule: &rule,
            injected: false,
        };
        let result = apply(&mut race, &rule, "loser", None, "loser-call").unwrap();
        assert_eq!(result["state"], "near_rules");
        let counts = race
            .reader
            .read_batch(&[q("SELECT count(*) FROM rules", vec![]), receipt("loser")])
            .unwrap();
        assert_eq!(number(&counts[0]).unwrap(), 1);
        assert!(counts[1].is_empty());
    }
}

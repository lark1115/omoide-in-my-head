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
fn now() -> Result<i64> {
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
            "INSERT INTO source_guard VALUES(CASE WHEN EXISTS(SELECT 1 FROM utterances WHERE id=?) THEN 1 ELSE NULL END)",
            vec![s(&rule.source)],
        ),
        q("DELETE FROM source_guard", vec![]),
        q(
            "INSERT INTO rules(id,version,state,body,scene,aliases,actions,target,purpose,exclusion,source,created,parent_id,pending_against) VALUES(?,?,?,?,?,?,?,?,?,?,?,?,?,?)",
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
pub fn apply(db: &mut impl Store, command: &Command, op: &str) -> Result<Json> {
    nonempty(op)?;
    let input = serde_json::to_vec(command).map_err(|e| Error::Input(e.to_string()))?;
    if input.len() > 1_900_000 || op.len() > 4096 {
        return Err(Error::Input("input exceeds storage limits".into()));
    }
    let hash = digest(input);
    let mut queries = vec![revision(), receipt(op)];
    match command {
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
            queries.extend([
                source(&rule.source),
                near_query(
                    &document(&rule.text, &rule.scene, &rule.aliases),
                    &json!(rule.actions).to_string(),
                ),
            ]);
            if let Some(version) = &rule.against {
                queries.push(relations::conflicts(version));
                queries.push(relations::adjacent_scopes(version));
            }
        }
        Command::Fact {
            command: FactCommand::Add {
                source: id, text, ..
            },
        } => {
            nonempty(text)?;
            queries.push(source(id));
        }
        Command::Resolve { pending, .. } => {
            queries.push(q("SELECT p.id,c.version,p.target FROM rules p JOIN rules c ON c.version=p.pending_against WHERE p.version=? AND p.state='pending' AND c.state='current'",vec![s(pending)]));
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
            Command::Fact {
                command:
                    FactCommand::Add {
                        source,
                        text,
                        target,
                        certainty,
                    },
            } => {
                required_record(&sets[2])?;
                (
                    vec![q(
                        "INSERT INTO facts(id,body,source,certainty,target,created) VALUES(?,?,?,?,?,?)",
                        vec![
                            s(&rid),
                            s(text),
                            s(source),
                            s(certainty),
                            opt(target),
                            Value::Integer(time),
                        ],
                    )],
                    json!({"state":"stored","kind":"fact","id":rid,"operation_id":op}),
                )
            }
            _ => unreachable!(),
        };
        changes.push(save_receipt(op, &hash, &result.to_string()));
        match db.exec_batch(&guarded(rev, changes)) {
            Ok(()) => return Ok(result),
            Err(Error::Revision) if start.elapsed() < Duration::from_secs(5) => {
                thread::sleep(Duration::from_millis(2))
            }
            Err(Error::Revision) => return Err(Error::Busy),
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
    fn racing_registration_rechecks_neighbors_before_activation() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("race.db");
        let mut db = Sqlite::open(&path, true).unwrap();
        db.initialize("fixture", &[]).unwrap();
        let source =
            Cli::parse_from(["omoide", "say", "--text", "A synthetic instruction"]).command;
        let source_id = apply(&mut db, &source, "source").unwrap()["id"]
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
                assert_eq!(queries.len(), 4);
                if !self.injected {
                    self.injected = true;
                    assert_eq!(
                        apply(&mut self.writer, self.rule, "winner")?["state"],
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
        let result = apply(&mut race, &rule, "loser").unwrap();
        assert_eq!(result["state"], "near_rules");
        let counts = race
            .reader
            .read_batch(&[q("SELECT count(*) FROM rules", vec![]), receipt("loser")])
            .unwrap();
        assert_eq!(number(&counts[0]).unwrap(), 1);
        assert!(counts[1].is_empty());
    }
}

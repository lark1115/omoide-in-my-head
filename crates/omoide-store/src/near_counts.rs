//! Document frequencies from current rules only; both entry points run inside the registration read batch.
use crate::{Statement, Value};
use omoide_core::Tokens;

fn counts(input: &str, args: Vec<Value>) -> Statement {
    Statement::new(
        format!(
            r#"WITH incoming(words,singles) AS ({input}),
        terms(col,term) AS (
          SELECT 'words',j.value FROM incoming,json_each('["'||replace(words,' ','","')||'"]') j WHERE j.value!=''
          UNION ALL SELECT 'singles',j.value FROM incoming,json_each('["'||replace(singles,' ','","')||'"]') j WHERE j.value!='')
        SELECT col,term,(SELECT count(*) FROM rule_index JOIN rules r ON r.rowid=rule_index.rowid
            WHERE r.state='current' AND r.kind='rule' AND rule_index MATCH (t.col||' : "'||t.term||'"')) FROM terms t
        UNION ALL SELECT 'total','',count(*) FROM rules WHERE state='current' AND kind='rule'"#
        ),
        args,
    )
}
pub fn proposal(t: &Tokens) -> Statement {
    let pack = |s: &std::collections::BTreeSet<String>| {
        Value::Text(s.iter().map(String::as_str).collect::<Vec<_>>().join(" "))
    };
    counts("SELECT ?1,?2", vec![pack(&t.words), pack(&t.singles)])
}
pub fn candidate(version: &str) -> Statement {
    counts(
        "SELECT i.words,i.singles FROM rule_index i JOIN rules r ON r.rowid=i.rowid WHERE r.version=?1",
        vec![Value::Text(version.to_owned())],
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::{Connection, params_from_iter};
    use std::collections::BTreeMap;
    // [REQ-RULE-1]
    #[test]
    fn draft_and_persisted_skill_get_current_rule_frequencies() {
        let db = Connection::open_in_memory().unwrap();
        db.execute_batch("CREATE TABLE rules(version TEXT,state TEXT,kind TEXT);
            CREATE VIRTUAL TABLE rule_index USING fts5(words,singles);
            INSERT INTO rules VALUES('a','current','rule'),('b','current','rule'),('c','pending','rule'),('d','superseded','rule'),('e','current','skill'),('f','candidate','skill');
            INSERT INTO rule_index(words,singles) VALUES('red 雨幕','雨 幕'),('red 月幕','月 幕'),('red','幕'),('red','幕'),('red','幕'),('red wax 雨幕','幕 雨');").unwrap();
        let read = |q: Statement| {
            db.prepare(&q.sql)
                .unwrap()
                .query_map(params_from_iter(&q.args), |r| {
                    Ok((
                        (r.get::<_, String>(0)?, r.get::<_, String>(1)?),
                        r.get::<_, usize>(2)?,
                    ))
                })
                .unwrap()
                .collect::<Result<BTreeMap<_, _>, _>>()
                .unwrap()
        };
        let counts = read(proposal(&omoide_core::tokens("red wax 雨幕")));
        assert_eq!(counts, read(candidate("f")));
        assert_eq!(counts[&("total".into(), "".into())], 2);
        assert_eq!(counts[&("words".into(), "red".into())], 2);
        assert_eq!(counts[&("words".into(), "wax".into())], 0);
        assert_eq!(counts[&("words".into(), "雨幕".into())], 1);
        assert_eq!(counts[&("singles".into(), "幕".into())], 2);
        assert_eq!(counts[&("singles".into(), "雨".into())], 1);
    }
}

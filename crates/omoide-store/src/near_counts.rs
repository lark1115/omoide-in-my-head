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

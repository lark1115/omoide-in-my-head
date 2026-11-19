use crate::{
    Error, Statement, Value,
    lookup::{SOURCE, expression},
};
use omoide_core::{Tokens, tokens};
use rusqlite::{Connection, params_from_iter};
use std::fmt::Write;

fn s(text: &str) -> Value {
    Value::Text(text.into())
}
fn opt(text: Option<&str>) -> Value {
    text.map_or(Value::Null, s)
}
pub const RECORD:&str="json_patch(json_object('id',m.record_id,'version_id',m.id,'kind',m.kind,'text',m.body,
    'target',m.target,'created',m.created,'source_id',m.source,'used_count',(SELECT count(*) FROM notices n WHERE n.version=m.id AND n.kind='used')),m.details)";
const SOURCE_JOINS: &str = "LEFT JOIN utterances u ON u.id=m.source
    LEFT JOIN memories origin ON origin.id=m.source AND origin.kind='experience'";
const ORIGIN:&str="CASE WHEN u.id IS NOT NULL THEN json_object('id',u.id,'kind','utterance','speaker',u.speaker,'text',u.body,'target',u.target,'created',u.created)
    WHEN origin.id IS NOT NULL THEN json_object('id',origin.id,'kind','experience','text',origin.body,'target',origin.target,'created',origin.created) END";

fn subject(target: &str) -> String {
    let mut encoded = String::from("t");
    for b in target.bytes() {
        write!(encoded, "{b:02x}").unwrap();
    }
    encoded
}
pub fn index(id: &str, body: &str, target: Option<&str>) -> Statement {
    let terms = tokens(body);
    Statement::new(
        "INSERT INTO evidence_index(rowid,words,singles,subject) VALUES((SELECT rowid FROM memories WHERE id=?),?,?,?)",
        vec![
            s(id),
            s(&terms.words.into_iter().collect::<Vec<_>>().join(" ")),
            s(&terms.singles.into_iter().collect::<Vec<_>>().join(" ")),
            s(&target.map(subject).unwrap_or_default()),
        ],
    )
}
pub fn show(id: &str) -> Statement {
    Statement::new(
        format!(
            "SELECT {RECORD},{ORIGIN} FROM memories m {SOURCE_JOINS} WHERE m.record_id IN (SELECT record_id FROM memories WHERE record_id=?1 OR id=?1) ORDER BY m.rowid"
        ),
        vec![s(id)],
    )
}
pub fn origin(id: &str) -> Statement {
    Statement::new(
        format!(
            "SELECT {SOURCE} FROM utterances u WHERE u.id=?1 UNION ALL SELECT {RECORD} FROM memories m WHERE id=?1 AND kind='experience'"
        ),
        vec![s(id)],
    )
}
pub fn migrate(tx: &Connection) -> Result<(), Error> {
    tx.execute_batch("ALTER TABLE meta RENAME TO old_meta;")?;
    tx.execute_batch(include_str!("schema.sql"))?;
    tx.execute_batch(include_str!("evidence.sql"))?;
    tx.execute_batch(
        "INSERT INTO meta SELECT singleton,revision+1,usage_revision,3,store_id FROM old_meta;
        DROP TABLE old_meta;
        INSERT INTO memories(id,record_id,kind,body,target,created,source,details)
        SELECT id,id,'fact',body,target,created,source,json_object('certainty',certainty) FROM facts;",
    )?;
    let mut legacy = tx.prepare("SELECT id,body,target FROM facts")?;
    let mut rows = legacy.query([])?;
    while let Some(r) = rows.next()? {
        let stmt = index(
            &r.get::<_, String>(0)?,
            &r.get::<_, String>(1)?,
            r.get::<_, Option<String>>(2)?.as_deref(),
        );
        tx.execute(&stmt.sql, params_from_iter(stmt.args))?;
    }
    drop(rows);
    drop(legacy);
    tx.execute_batch("DROP TABLE facts;")?;
    Ok(())
}
pub struct Filter<'a> {
    pub terms: &'a Tokens,
    pub target: Option<&'a str>,
    pub kind: Option<&'a str>,
    pub period: Option<&'a str>,
    pub scope_json: &'a str,
    pub claim: Option<&'a str>,
    pub limit: u32,
}
pub fn candidates(filter: Filter<'_>) -> Statement {
    let mut lexical = expression(filter.terms);
    if lexical.is_empty() {
        lexical = "words : \"\"".into();
    }
    let subject = filter
        .target
        .map(|v| format!("subject : \"{}\"", subject(v)))
        .unwrap_or_else(|| "subject : \"\"".into());
    // Both routes go through FTS; target-based evidence cannot crowd lexical hits out of the pool.
    Statement::new(format!("WITH hits AS (
      SELECT rowid,1 AS lexical,rank AS relevance FROM evidence_index WHERE evidence_index MATCH ?1
      UNION ALL SELECT rowid,0,0 FROM evidence_index WHERE evidence_index MATCH ?2),
      pool AS (SELECT rowid,max(lexical) AS lexical,min(relevance) AS relevance FROM hits GROUP BY rowid)
      SELECT {RECORD},CASE WHEN m.kind='decision' THEN {ORIGIN} END,pool.lexical FROM pool JOIN memories m ON m.rowid=pool.rowid {SOURCE_JOINS}
      WHERE coalesce(json_extract(m.details,'$.decision'),'') <> 'revised' AND (?3 IS NULL OR m.target IS NULL OR m.target=?3) AND (?4 IS NULL OR m.kind=?4)
        AND (?5 IS NULL OR json_extract(m.details,'$.period')=?5)
        AND (?6='[]' OR EXISTS(SELECT 1 FROM json_each(m.details,'$.scope') a JOIN json_each(?6) b ON a.value=b.value))
        AND (?7 IS NULL OR json_extract(m.details,'$.claim')=?7)
      ORDER BY lexical DESC, CASE WHEN lexical=0 THEN (json_extract(m.details,'$.decision') IS 'decided' OR json_extract(m.details,'$.certainty') IS 'verified') ELSE 0 END DESC,
        relevance,m.created DESC,m.id LIMIT ?8"),
      vec![s(&lexical),s(&subject),opt(filter.target),opt(filter.kind),opt(filter.period),s(filter.scope_json),opt(filter.claim),Value::Integer(i64::from(filter.limit)+1)])
}

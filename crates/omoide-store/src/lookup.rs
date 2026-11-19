use super::{Statement, Value, revision};
use omoide_core::{Tokens, single_japanese};
fn q(sql: impl Into<String>, args: Vec<Value>) -> Statement {
    Statement::new(sql, args)
}
fn s(v: &str) -> Value {
    Value::Text(v.into())
}
fn val(v: Option<&str>) -> Value {
    v.map_or(Value::Null, s)
}
pub const RULE: &str = "json_object('id',r.id,'version_id',r.version,'kind','rule','state',r.state,'text',r.body,'scene',r.scene,'aliases',json(r.aliases),'actions',json(r.actions),'target',r.target,'purpose',r.purpose,'exclusion',r.exclusion,'source_id',r.source,'created',r.created)";
pub const SOURCE: &str = "json_object('id',u.id,'kind','utterance','text',u.body,'speaker',u.speaker,'target',u.target,'created',u.created)";
const ELIGIBLE: &str = "r.state='current' AND (r.target IS NULL OR r.target=?1) AND (r.exclusion IS NULL OR r.exclusion IS NOT ?1)";
pub fn expression(t: &Tokens) -> String {
    t.words
        .iter()
        .map(|w| {
            format!(
                "{} : \"{}\"",
                if single_japanese(w) {
                    "singles"
                } else {
                    "words"
                },
                w
            )
        })
        .collect::<Vec<_>>()
        .join(" OR ")
}
pub fn candidates(
    terms: &Tokens,
    target: Option<&str>,
    action: Option<&str>,
    scan_limit: u32,
    fts_limit: u32,
) -> Vec<Statement> {
    let expr = expression(terms);
    let mut queries = vec![
        revision(),
        q(
            format!(
                "SELECT {RULE} FROM action_links a JOIN rules r ON r.id=a.rule_id WHERE {ELIGIBLE} AND a.action=?2"
            ),
            vec![val(target), val(action)],
        ),
        q(
            format!(
                "SELECT {RULE} FROM rules r WHERE {ELIGIBLE} AND ?2 AND (SELECT count(*) FROM rules WHERE state='current')<=?3"
            ),
            vec![
                val(target),
                Value::Integer(i64::from(!expr.is_empty())),
                Value::Integer(scan_limit.into()),
            ],
        ),
    ];
    if !expr.is_empty() {
        let sql = format!(
            "SELECT {RULE}
            FROM rule_index JOIN rules r ON r.rowid=rule_index.rowid
            WHERE {ELIGIBLE}
              AND (SELECT count(*) FROM rules WHERE state='current')>?2
              AND rule_index MATCH ?3
            ORDER BY rank,r.id LIMIT ?4"
        );
        let limit_with_sentinel = Value::Integer(i64::from(fts_limit) + 1);
        queries.push(q(
            sql,
            vec![
                val(target),
                Value::Integer(scan_limit.into()),
                s(&expr),
                limit_with_sentinel,
            ],
        ));
    }
    queries
}
pub fn near_query(t: &Tokens, actions_json: &str) -> Statement {
    let mut where_sql="r.id IN (SELECT rule_id FROM action_links WHERE action IN (SELECT value FROM json_each(?1)))".to_string();
    let mut args = vec![s(actions_json)];
    if !t.words.is_empty() {
        where_sql
            .push_str(" OR r.rowid IN (SELECT rowid FROM rule_index WHERE rule_index MATCH ?2)");
        args.push(s(&expression(t)));
    }
    q(
        format!(
            "SELECT {RULE},{SOURCE} FROM rules r JOIN utterances u ON u.id=r.source WHERE r.state='current' AND ({where_sql}) ORDER BY r.version"
        ),
        args,
    )
}
pub fn index_rule(id: &str, words: &str, singles: &str) -> Statement {
    q(
        "INSERT INTO rule_index(rowid,words,singles) VALUES((SELECT rowid FROM rules WHERE id=?),?,?)",
        vec![s(id), s(words), s(singles)],
    )
}

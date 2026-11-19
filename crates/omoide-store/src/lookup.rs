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
pub const RULE: &str = "json_object('id',r.id,'version_id',r.version,'kind','rule','state',r.state,'text',r.body,'scene',r.scene,'aliases',json(r.aliases),'actions',json(r.actions),'target',r.target,'purpose',r.purpose,'exclusion',r.exclusion,'source_id',r.source,'created',r.created,'used_count',(SELECT count(*) FROM notices n WHERE n.version=r.version AND n.kind='used'),'successor',r.successor,'parent_id',r.parent_id,'pending_against',r.pending_against)";
pub const SOURCE: &str = "json_object('id',u.id,'kind','utterance','text',u.body,'speaker',u.speaker,'target',u.target,'created',u.created)";
const ELIGIBLE: &str = "r.state='current' AND (?1 IS NULL OR r.target IS NULL OR r.target=?1) AND (r.exclusion IS NULL OR r.exclusion IS NOT ?1)";
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
                "SELECT {RULE} FROM action_links a JOIN rules r ON r.version=a.version WHERE {ELIGIBLE} AND a.action=?2"
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
    let mut where_sql="r.version IN (SELECT version FROM action_links WHERE action IN (SELECT value FROM json_each(?1)))".to_string();
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
        "INSERT INTO rule_index(rowid,words,singles) VALUES((SELECT rowid FROM rules WHERE version=?),?,?)",
        vec![s(id), s(words), s(singles)],
    )
}

/// Fetch link endpoints in the same snapshot as the ordinary candidates.
/// The partial indexes restrict these reads to the exceptional and pending rows.
pub fn linked_rules(target: Option<&str>, action: Option<&str>, terms: &Tokens) -> [Statement; 2] {
    let eligible_scope = |a: &str| {
        format!(
            "(?1 IS NULL OR {a}.target IS NULL OR {a}.target=?1)
        AND ({a}.exclusion IS NULL OR {a}.exclusion IS NOT ?1)"
        )
    };
    // Use the same normalized vocabulary as the ordinary pool. A hit on a pending
    // version seeds its family even when the incumbent has no lexical overlap.
    let mut seeds = "SELECT version FROM action_links WHERE action=?2".to_owned();
    let mut args = vec![val(target), val(action)];
    if !terms.words.is_empty() {
        seeds.push_str(" UNION SELECT r.version FROM rule_index JOIN rules r ON r.rowid=rule_index.rowid WHERE rule_index MATCH ?3");
        args.push(s(&expression(terms)));
    }
    let families=format!("WITH RECURSIVE direct(version) AS ({seeds}), families(id) AS (
        SELECT r.id FROM rules r WHERE r.version IN direct AND r.state IN ('current','pending') AND {scope}
        UNION SELECT r.id FROM rules r JOIN families f ON f.id=r.parent_id WHERE r.state='current' AND {scope})",scope=eligible_scope("r"));
    let exceptions=q(format!("{families} SELECT {RULE} FROM rules r
        WHERE r.state='current' AND r.parent_id IS NOT NULL AND {} AND r.parent_id IN families ORDER BY r.version",eligible_scope("r")),args.clone());
    let pending = RULE.replace("r.", "p.");
    let pending_source = SOURCE.replace("u.", "v.");
    let conflicts=q(format!("{families} SELECT {RULE},{SOURCE},{pending},{pending_source}
        FROM rules p JOIN rules r ON r.version=p.pending_against
        JOIN utterances u ON u.id=r.source JOIN utterances v ON v.id=p.source
        WHERE p.state='pending' AND r.state='current' AND {} AND {} AND r.id IN families ORDER BY p.version",
        eligible_scope("p"),eligible_scope("r")),args);
    [exceptions, conflicts]
}

use omoide_store::{Error, Rows, Statement, Value};
use serde_json::{Value as Json, json};
use sha2::{Digest, Sha256};
pub type Result<T> = std::result::Result<T, Error>;
pub fn digest(bytes: impl AsRef<[u8]>) -> String {
    format!("{:x}", Sha256::digest(bytes))
}
pub fn s(v: &str) -> Value {
    Value::Text(v.to_owned())
}
pub fn opt(v: &Option<String>) -> Value {
    v.as_deref().map_or(Value::Null, s)
}
pub fn q(sql: impl Into<String>, args: Vec<Value>) -> Statement {
    Statement::new(sql, args)
}
pub fn text(v: &Value) -> Result<&str> {
    match v {
        Value::Text(s) => Ok(s),
        _ => Err(Error::Unknown("expected text from storage".into())),
    }
}
pub fn int(v: &Value) -> Result<i64> {
    match v {
        Value::Integer(n) => Ok(*n),
        _ => Err(Error::Unknown("expected integer from storage".into())),
    }
}
pub fn number(rows: &Rows) -> Result<i64> {
    rows.first()
        .and_then(|r| r.first())
        .ok_or_else(|| Error::Unknown("missing metadata".into()))
        .and_then(int)
}
pub fn parse(v: &Value) -> Result<Json> {
    serde_json::from_str(text(v)?).map_err(|e| Error::Unknown(e.to_string()))
}
use omoide_store::lookup::{RULE, SOURCE};
pub fn source(id: &str) -> Statement {
    q(
        format!("SELECT {SOURCE} FROM utterances u WHERE id=?"),
        vec![s(id)],
    )
}
pub fn required_record(rows: &Rows) -> Result<Json> {
    rows.first()
        .ok_or_else(|| Error::Input("record not found".into()))
        .and_then(|r| parse(&r[0]))
}
pub fn show(db: &mut impl omoide_store::Store, id: &str, history: bool) -> Result<Json> {
    let family = "r.id IN (SELECT id FROM rules WHERE id=?1 OR version=?1)";
    let versions = q(
        format!(
            "SELECT {RULE},{SOURCE} FROM rules r JOIN utterances u ON u.id=r.source WHERE {family} ORDER BY r.rowid"
        ),
        vec![s(id)],
    );
    let extra = q(
        format!(
            "SELECT r.version,{SOURCE} FROM rules r JOIN extra_sources e ON e.version=r.version JOIN utterances u ON u.id=e.source WHERE {family} ORDER BY r.rowid,u.created,u.id"
        ),
        vec![s(id)],
    );
    let fact = q(
        format!(
            "SELECT json_object('kind','fact','id',f.id,'text',f.body,'certainty',f.certainty,'created',f.created,'target',f.target,'source_id',f.source),{SOURCE} FROM facts f JOIN utterances u ON u.id=f.source WHERE f.id=?"
        ),
        vec![s(id)],
    );
    let sets = db.read_batch(&[source(id), versions, extra, fact])?;
    if let Some(row) = sets[3].first() {
        let source = parse(&row[1])?;
        return Ok(json!({"record":parse(&row[0])?,"source":source,"sources":[source]}));
    }
    if sets[1].is_empty() {
        return Ok(json!({"record":required_record(&sets[0])?}));
    }
    let mut entries = Vec::new();
    for row in &sets[1] {
        let record = parse(&row[0])?;
        let primary = parse(&row[1])?;
        let mut sources = vec![primary.clone()];
        for extra in &sets[2] {
            if record["version_id"] == text(&extra[0])? {
                sources.push(parse(&extra[1])?)
            }
        }
        entries.push(json!({"record":record,"source":primary,"sources":sources}));
    }
    let conflicts: Vec<_> = entries
        .iter()
        .filter(|e| e["record"]["state"] == "pending")
        .filter_map(|pending| {
            entries
                .iter()
                .find(|e| e["record"]["version_id"] == pending["record"]["pending_against"])
                .map(|current| json!({"pending":pending,"current":current}))
        })
        .collect();
    let entry = entries
        .iter()
        .find(|e| e["record"]["version_id"] == id)
        .or_else(|| entries.iter().find(|e| e["record"]["state"] == "current"))
        .ok_or_else(|| Error::Unknown("record has no current version".into()))?;
    let mut result = entry.clone();
    if !conflicts.is_empty() {
        result["unresolved_conflicts"] = json!(conflicts);
    }
    if history {
        result["history"] = json!(entries);
    }
    Ok(result)
}

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
pub fn show(db: &mut impl omoide_store::Store, id: &str) -> Result<Json> {
    let rule = q(
        format!(
            "SELECT {RULE},{SOURCE} FROM rules r
        JOIN utterances u ON u.id=r.source WHERE r.id=?1 OR r.version=?1"
        ),
        vec![s(id)],
    );
    let rows = db.read_batch(&[source(id), rule])?;
    if let Some(r) = rows[1].first() {
        return Ok(json!({"record":parse(&r[0])?,"source":parse(&r[1])?}));
    }
    Ok(json!({"record":required_record(&rows[0])?}))
}

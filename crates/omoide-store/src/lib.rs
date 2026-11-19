pub mod lookup;
pub use rusqlite::types::Value;
use rusqlite::{Connection, OpenFlags, TransactionBehavior, limits::Limit, params_from_iter};
use std::{path::Path, time::Duration};

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("revision changed")]
    Revision,
    #[error("writer wait limit exceeded")]
    Busy,
    #[error("invalid input: {0}")]
    Input(String),
    #[error("storage full")]
    Full,
    #[error("storage failure or outcome unknown: {0}")]
    Unknown(String),
}
impl From<rusqlite::Error> for Error {
    fn from(e: rusqlite::Error) -> Self {
        use rusqlite::ErrorCode::*;
        match &e {
            rusqlite::Error::SqliteFailure(code, message) => match code.code {
                DatabaseBusy | DatabaseLocked => Self::Busy,
                DiskFull => Self::Full,
                ConstraintViolation
                    if message.as_deref()
                        == Some("NOT NULL constraint failed: revision_guard.ok") =>
                {
                    Self::Revision
                }
                ConstraintViolation | TooBig | ParameterOutOfRange => Self::Input(e.to_string()),
                _ => Self::Unknown(e.to_string()),
            },
            _ => Self::Unknown(e.to_string()),
        }
    }
}
pub struct Statement {
    pub sql: String,
    pub args: Vec<Value>,
}
impl Statement {
    fn validate(&self) -> Result<(), Error> {
        if self.args.len() > 100 || self.sql.len() > 100_000 {
            return Err(Error::Input(
                "statement exceeds portable storage limits".into(),
            ));
        }
        Ok(())
    }
    pub fn new(sql: impl Into<String>, args: Vec<Value>) -> Self {
        Self {
            sql: sql.into(),
            args,
        }
    }
}
pub type Rows = Vec<Vec<Value>>;
pub trait Store {
    fn read_batch(&mut self, queries: &[Statement]) -> Result<Vec<Rows>, Error>;
    fn exec_batch(&mut self, statements: &[Statement]) -> Result<(), Error>;
}
pub struct Sqlite {
    conn: Connection,
}
impl Sqlite {
    pub fn open(path: &Path, create: bool) -> Result<Self, Error> {
        let mut flags = OpenFlags::SQLITE_OPEN_READ_WRITE;
        if create {
            flags |= OpenFlags::SQLITE_OPEN_CREATE;
        }
        let conn = Connection::open_with_flags(path, flags)?;
        conn.busy_timeout(Duration::from_secs(2))?;
        conn.pragma_update(None, "foreign_keys", "ON")?;
        conn.set_limit(Limit::SQLITE_LIMIT_LENGTH, 2_000_000)?;
        conn.set_limit(Limit::SQLITE_LIMIT_SQL_LENGTH, 100_000)?;
        conn.set_limit(Limit::SQLITE_LIMIT_VARIABLE_NUMBER, 100)?;
        Ok(Self { conn })
    }
    /// Bootstrap an empty file. The first logical write and its receipt commit with the schema.
    /// A competing initializer may finish first; in that case use the normal replay path.
    pub fn initialize(&mut self, store_id: &str, first_write: &[Statement]) -> Result<bool, Error> {
        fn existing(conn: &Connection) -> Result<bool, Error> {
            let tables: i64 = conn.query_row(
                "SELECT count(*) FROM sqlite_schema WHERE type='table'",
                [],
                |r| r.get(0),
            )?;
            if tables == 0 {
                return Ok(false);
            }
            if conn
                .query_row(
                    "SELECT schema_version FROM meta WHERE singleton=1",
                    [],
                    |r| r.get::<_, i64>(0),
                )
                .ok()
                != Some(1)
            {
                return Err(Error::Input("unrecognized store or schema version".into()));
            }
            Ok(true)
        }
        if existing(&self.conn)? {
            return Ok(false);
        }
        self.conn.pragma_update(None, "journal_mode", "WAL")?;
        let tx = self
            .conn
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        if existing(&tx)? {
            return Ok(false);
        }
        tx.execute_batch(include_str!("schema.sql"))?;
        tx.execute("INSERT INTO meta VALUES(1,0,0,1,?)", [store_id])?;
        for stmt in first_write {
            Self::write_statement(&tx, stmt)?;
        }
        tx.commit()?;
        Ok(true)
    }
    fn write_statement(tx: &Connection, s: &Statement) -> Result<(), Error> {
        s.validate()?;
        let verb = s
            .sql
            .split_ascii_whitespace()
            .next()
            .unwrap_or_default()
            .to_ascii_uppercase();
        if !matches!(verb.as_str(), "INSERT" | "UPDATE" | "DELETE") {
            return Err(Error::Input(
                "expected one data modification statement".into(),
            ));
        }
        tx.execute(&s.sql, params_from_iter(&s.args))?;
        Ok(())
    }
    fn read_observed(
        &mut self,
        queries: &[Statement],
        mut after: impl FnMut(usize),
    ) -> Result<Vec<Rows>, Error> {
        let tx = self.conn.transaction()?;
        let mut sets = Vec::new();
        for (i, q) in queries.iter().enumerate() {
            q.validate()?;
            let mut stmt = tx.prepare(&q.sql)?;
            if !stmt.readonly()
                || !matches!(
                    q.sql
                        .split_ascii_whitespace()
                        .next()
                        .unwrap_or_default()
                        .to_ascii_uppercase()
                        .as_str(),
                    "SELECT" | "WITH"
                )
            {
                return Err(Error::Input("write in read_batch".into()));
            }
            let count = stmt.column_count();
            let rows = stmt.query_map(params_from_iter(&q.args), |r| {
                (0..count)
                    .map(|i| r.get(i))
                    .collect::<rusqlite::Result<Vec<Value>>>()
            })?;
            sets.push(rows.collect::<rusqlite::Result<Rows>>()?);
            after(i);
        }
        tx.commit()?;
        Ok(sets)
    }
    fn exec_observed(
        &mut self,
        statements: &[Statement],
        mut after: impl FnMut(usize),
    ) -> Result<(), Error> {
        let tx = self
            .conn
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        for (i, s) in statements.iter().enumerate() {
            Self::write_statement(&tx, s)?;
            after(i);
        }
        tx.commit()?;
        Ok(())
    }
}
impl Store for Sqlite {
    fn read_batch(&mut self, queries: &[Statement]) -> Result<Vec<Rows>, Error> {
        self.read_observed(queries, |_| {})
    }
    fn exec_batch(&mut self, statements: &[Statement]) -> Result<(), Error> {
        self.exec_observed(statements, |_| {})
    }
}
pub fn revision() -> Statement {
    Statement::new("SELECT revision FROM meta WHERE singleton=1", vec![])
}
pub fn guarded(rev: i64, changes: Vec<Statement>) -> Vec<Statement> {
    let mut batch = vec![
        Statement::new(
            "INSERT INTO revision_guard VALUES(CASE WHEN (SELECT revision FROM meta WHERE singleton=1)=? THEN 1 ELSE NULL END)",
            vec![Value::Integer(rev)],
        ),
        Statement::new("DELETE FROM revision_guard", vec![]),
    ];
    batch.extend(changes);
    batch.push(Statement::new(
        "UPDATE meta SET revision=revision+1 WHERE singleton=1",
        vec![],
    ));
    batch
}
pub fn receipt(op: &str) -> Statement {
    Statement::new(
        "SELECT digest,result FROM receipts WHERE op=?",
        vec![Value::Text(op.into())],
    )
}
pub fn save_receipt(op: &str, digest: &str, result: &str) -> Statement {
    Statement::new(
        "INSERT INTO receipts VALUES(?,?,?)",
        [op, digest, result].map(|s| Value::Text(s.into())).to_vec(),
    )
}
#[cfg(test)]
mod tests;

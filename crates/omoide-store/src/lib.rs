pub mod evidence;
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
        let mut conn = Connection::open_with_flags(path, flags)?;
        conn.busy_timeout(Duration::from_secs(2))?;
        conn.pragma_update(None, "foreign_keys", "ON")?;
        conn.set_limit(Limit::SQLITE_LIMIT_LENGTH, 2_000_000)?;
        conn.set_limit(Limit::SQLITE_LIMIT_SQL_LENGTH, 100_000)?;
        conn.set_limit(Limit::SQLITE_LIMIT_VARIABLE_NUMBER, 100)?;
        if matches!(Self::schema_version(&conn)?, Some(1..=3)) {
            let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
            if Self::schema_version(&tx)? == Some(1) {
                tx.execute_batch(include_str!("migrate_v1_before.sql"))?;
                tx.execute_batch(include_str!("schema_v2.sql"))?;
                tx.execute_batch(include_str!("migrate_v1_after.sql"))?;
            }
            if Self::schema_version(&tx)? == Some(2) {
                evidence::migrate(&tx)?;
            }
            if Self::schema_version(&tx)? == Some(3) {
                tx.execute_batch(include_str!("migrate_v3.sql"))?;
                tx.execute_batch(include_str!("schema.sql"))?;
                tx.execute_batch(include_str!("upgrade_v4.sql"))?;
            }
            tx.commit()?;
        }
        Ok(Self { conn })
    }
    /// Hold the SQLite writer reservation across the final metadata check and filesystem rename.
    /// No database data is modified. Slow rendering and temporary-file writes happen before this.
    pub fn publish_if_current(
        &mut self,
        store: &str,
        revision: i64,
        usage: i64,
        install: impl FnOnce() -> Result<(), Error>,
    ) -> Result<bool, Error> {
        let tx = self
            .conn
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let current: bool = tx.query_row(
            "SELECT store_id=? AND revision=? AND usage_revision=? FROM meta WHERE singleton=1",
            rusqlite::params![store, revision, usage],
            |r| r.get(0),
        )?;
        if current {
            install()?;
        }
        tx.commit()?;
        Ok(current)
    }
    fn schema_version(conn: &Connection) -> Result<Option<i64>, Error> {
        let populated: bool = conn.query_row(
            "SELECT EXISTS(SELECT 1 FROM sqlite_schema WHERE type='table')",
            [],
            |r| r.get(0),
        )?;
        if !populated {
            return Ok(None);
        }
        let v: i64 = conn.query_row(
            "SELECT schema_version FROM meta WHERE singleton=1",
            [],
            |r| r.get(0),
        )?;
        if !matches!(v, 1..=4) {
            return Err(Error::Input(format!("unsupported store schema {v}")));
        }
        Ok(Some(v))
    }
    /// Bootstrap an empty file. The first logical write and its receipt commit with the schema.
    /// A competing initializer may finish first; in that case use the normal replay path.
    pub fn initialize(&mut self, store_id: &str, first_write: &[Statement]) -> Result<bool, Error> {
        if Self::schema_version(&self.conn)?.is_some() {
            return Ok(false);
        }
        self.conn.pragma_update(None, "journal_mode", "WAL")?;
        let tx = self
            .conn
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        if Self::schema_version(&tx)?.is_some() {
            return Ok(false);
        }
        tx.execute_batch(include_str!("schema.sql"))?;
        tx.execute_batch(include_str!("evidence.sql"))?;
        tx.execute("INSERT INTO meta VALUES(1,0,0,4,?)", [store_id])?;
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
            // Used only by the opt-in process interruption harness; the default build has no hook.
            #[cfg(all(unix, feature = "fault-injection"))]
            if stop_for_test(
                &s.sql,
                std::env::var_os("OMOIDE_TEST_STOP_BEFORE_COMMIT").is_some(),
                std::env::var_os("OMOIDE_TEST_STOP_CALL_LOG").is_some(),
            ) {
                // SAFETY: raise targets only this process; SIGSTOP has no signal handler.
                unsafe {
                    libc::raise(libc::SIGSTOP);
                }
            }
            after(i);
        }
        tx.commit()?;
        Ok(())
    }
}
#[cfg(any(test, feature = "fault-injection"))]
fn stop_for_test(sql: &str, receipt: bool, log: bool) -> bool {
    (receipt && sql.starts_with("INSERT INTO receipts"))
        || (log && sql.starts_with("INSERT INTO calls"))
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
    guarded_column("revision", rev, changes)
}
pub fn guarded_usage(rev: i64, changes: Vec<Statement>) -> Vec<Statement> {
    guarded_column("usage_revision", rev, changes)
}
fn guarded_column(column: &str, rev: i64, changes: Vec<Statement>) -> Vec<Statement> {
    let mut batch = vec![
        Statement::new(
            format!(
                "INSERT INTO revision_guard VALUES(CASE WHEN (SELECT {column} FROM meta WHERE singleton=1)=? THEN 1 ELSE NULL END)"
            ),
            vec![Value::Integer(rev)],
        ),
        Statement::new("DELETE FROM revision_guard", vec![]),
    ];
    batch.extend(changes);
    batch.push(Statement::new(
        format!("UPDATE meta SET {column}={column}+1 WHERE singleton=1"),
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

pub mod near_counts;

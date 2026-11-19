use super::*;
use std::{
    fs,
    process::{Command, Stdio},
    thread,
    time::Instant,
};
fn sql(s: &str) -> Statement {
    Statement::new(s, vec![])
}
fn integer(row: &[Value]) -> i64 {
    match row[0] {
        Value::Integer(n) => n,
        _ => panic!("expected integer"),
    }
}
fn setup(dir: &Path) -> Sqlite {
    let mut db = Sqlite::open(&dir.join("test.db"), true).unwrap();
    db.initialize("test", &[]).unwrap();
    db.conn
        .execute_batch("CREATE TABLE counter(n INTEGER NOT NULL); INSERT INTO counter VALUES(0);")
        .unwrap();
    db
}
// [REQ-STORE-1]
#[test]
fn state_and_revision_share_one_snapshot() {
    let dir = tempfile::tempdir().unwrap();
    let mut a = setup(dir.path());
    let mut b = Sqlite::open(&dir.path().join("test.db"), false).unwrap();
    let snapshot = a
        .read_observed(&[sql("SELECT n FROM counter"), revision()], |i| {
            if i == 0 {
                b.exec_batch(&guarded(0, vec![sql("UPDATE counter SET n=1")]))
                    .unwrap();
            }
        })
        .unwrap();
    let state = integer(&snapshot[0][0]);
    let rev = integer(&snapshot[1][0]);
    assert_eq!((state, rev), (0, 0));
    assert!(matches!(
        a.exec_batch(&guarded(rev, vec![sql("UPDATE counter SET n=9")])),
        Err(Error::Revision)
    ));
    let rows = a.read_batch(&[sql("SELECT n FROM counter")]).unwrap();
    assert_eq!(integer(&rows[0][0]), 1);
}
// [REQ-STORE-1]
#[test]
fn missing_revision_fails_closed() {
    let dir = tempfile::tempdir().unwrap();
    let mut db = setup(dir.path());
    db.exec_batch(&[sql("DELETE FROM meta")]).unwrap();
    assert!(matches!(
        db.exec_batch(&guarded(0, vec![sql("UPDATE counter SET n=42")])),
        Err(Error::Revision)
    ));
}
// [REQ-STORE-1]
#[test]
#[ignore = "child entry point for contention_kill_and_replay"]
fn process_worker() {
    let path = std::env::var("OMOIDE_TEST_DIR").expect("run through contention_kill_and_replay");
    let dir = Path::new(&path);
    let worker = std::env::var("OMOIDE_TEST_WORKER").unwrap();
    let mut db = Sqlite::open(&dir.join("test.db"), false).unwrap();
    for idx in 0..20 {
        let op = format!("{worker}-{idx}");
        loop {
            let sets = db
                .read_batch(&[sql("SELECT n FROM counter"), revision(), receipt(&op)])
                .unwrap();
            if !sets[2].is_empty() {
                break;
            }
            let next = integer(&sets[0][0]) + 1;
            let rev = integer(&sets[1][0]);
            let batch = guarded(
                rev,
                vec![
                    Statement::new("UPDATE counter SET n=?", vec![Value::Integer(next)]),
                    save_receipt(&op, "digest", &next.to_string()),
                ],
            );
            let result = db.exec_observed(&batch, |i| {
                if worker == "kill" && idx == 3 && i == 2 {
                    fs::write(dir.join("in-transaction"), b"ready").unwrap();
                    loop {
                        thread::sleep(Duration::from_millis(50));
                    }
                }
            });
            match result {
                Ok(()) => break,
                Err(Error::Revision) => thread::yield_now(),
                other => panic!("{other:?}"),
            }
        }
    }
}
// [REQ-STORE-1]
#[test]
fn contention_kill_and_replay() {
    let dir = tempfile::tempdir().unwrap();
    let mut db = setup(dir.path());
    let spawn = |id: &str| {
        Command::new(std::env::current_exe().unwrap())
            .args(["--ignored", "--exact", "tests::process_worker"])
            .env("OMOIDE_TEST_DIR", dir.path())
            .env("OMOIDE_TEST_WORKER", id)
            .stdout(Stdio::null())
            .spawn()
            .unwrap()
    };
    let mut victim = spawn("kill");
    let mut workers: Vec<_> = ["a", "b", "c", "d"].map(spawn).into_iter().collect();
    let start = Instant::now();
    while !dir.path().join("in-transaction").exists() {
        assert!(start.elapsed() < Duration::from_secs(15));
        thread::sleep(Duration::from_millis(5));
    }
    victim.kill().unwrap();
    assert!(!victim.wait().unwrap().success());
    for w in &mut workers {
        assert!(w.wait().unwrap().success())
    }
    assert!(spawn("a").wait().unwrap().success());
    let sets = db
        .read_batch(&[
            sql("SELECT n FROM counter"),
            revision(),
            sql("SELECT count(*) FROM receipts"),
            receipt("kill-3"),
        ])
        .unwrap();
    for rows in &sets[..3] {
        assert_eq!(integer(&rows[0]), 83);
    }
    assert!(sets[3].is_empty());
}

// [REQ-STORE-1]
#[test]
fn transaction_control_is_not_a_batch_statement() {
    let dir = tempfile::tempdir().unwrap();
    let mut db = setup(dir.path());
    assert!(
        db.exec_batch(&[sql("UPDATE counter SET n=2"), sql("COMMIT")])
            .is_err()
    );
    assert!(db.read_batch(&[sql("COMMIT")]).is_err());
    assert_eq!(
        integer(&db.read_batch(&[sql("SELECT n FROM counter")]).unwrap()[0][0]),
        0
    );
}

// [REQ-STORE-1]
#[test]
fn initialization_rolls_back_schema_with_first_write() {
    let dir = tempfile::tempdir().unwrap();
    let mut db = Sqlite::open(&dir.path().join("new.db"), true).unwrap();
    assert!(
        db.initialize("aborted", &[sql("INSERT INTO source_guard VALUES(NULL)")])
            .is_err()
    );
    let tables = db
        .read_batch(&[sql("SELECT count(*) FROM sqlite_schema WHERE type='table'")])
        .unwrap();
    assert_eq!(integer(&tables[0][0]), 0);
    assert!(db.initialize("committed", &[]).unwrap());
    assert!(!db.initialize("ignored", &[]).unwrap());
}

// [REQ-STORE-1]
#[test]
fn portable_limits_reject_the_whole_batch() {
    let dir = tempfile::tempdir().unwrap();
    let mut db = setup(dir.path());
    let invalid = [
        Statement::new(
            format!("INSERT INTO counter VALUES(?) {}", " ".repeat(100_000)),
            vec![Value::Integer(1)],
        ),
        Statement::new(
            "INSERT INTO counter VALUES(?)",
            vec![Value::Integer(1); 101],
        ),
        Statement::new(
            "INSERT INTO counter VALUES(?)",
            vec![Value::Text("x".repeat(2_000_001))],
        ),
    ];
    for statement in invalid {
        assert!(matches!(
            db.exec_batch(&[sql("UPDATE counter SET n=1"), statement]),
            Err(Error::Input(_))
        ));
        assert_eq!(
            integer(&db.read_batch(&[sql("SELECT n FROM counter")]).unwrap()[0][0]),
            0
        );
    }
}

// [REQ-STORE-1]
#[test]
fn migrate_v1_preserves_identity_index_sources_and_receipts() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("v1.db");
    let old = Connection::open(&path).unwrap();
    old.execute_batch(include_str!("../tests/schema_v1.sql"))
        .unwrap();
    old.execute_batch("INSERT INTO meta VALUES(1,7,2,1,'fixture');
        INSERT INTO utterances VALUES('src','original','user',12,NULL);
        INSERT INTO rules VALUES(71,'record','v1','current','paper','sort','[]','[\"file\"]',NULL,NULL,NULL,'src',13);
        INSERT INTO rule_index(rowid,words,singles) VALUES(71,'paper sort','');
        INSERT INTO action_links VALUES('file','record');
        INSERT INTO receipts VALUES('first','digest','original result');").unwrap();
    drop(old);
    for _ in 0..2 {
        let mut db = Sqlite::open(&path, false).unwrap();
        let sets=db.read_batch(&[sql("SELECT schema_version,revision,usage_revision,store_id FROM meta"),
            sql("SELECT r.id,r.version,r.source,a.action FROM rule_index JOIN rules r ON r.rowid=rule_index.rowid
                 JOIN action_links a ON a.version=r.version WHERE rule_index MATCH 'paper'"),receipt("first")]).unwrap();
        assert_eq!(
            sets[0][0],
            [
                Value::Integer(3),
                Value::Integer(9),
                Value::Integer(2),
                Value::Text("fixture".into())
            ]
        );
        assert_eq!(
            sets[1][0],
            ["record", "v1", "src", "file"].map(|v| Value::Text(v.into()))
        );
        assert_eq!(
            sets[2][0],
            ["digest", "original result"].map(|v| Value::Text(v.into()))
        );
    }
}

// [REQ-STORE-1]
#[test]
fn migrate_v2_summary_preserves_source_and_builds_search_index() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("memory.db");
    let old = Connection::open(&path).unwrap();
    old.execute_batch(include_str!("schema_v2.sql")).unwrap();
    old.execute_batch(
        "INSERT INTO meta VALUES(1,4,7,2,'fixture');
        INSERT INTO utterances VALUES('original','all the conditions','user',12,'workshop');
        INSERT INTO facts VALUES('summary','ＰＡＰＥＲ 紙箱','original','reported','press',13);
        INSERT INTO receipts VALUES('old-request','old-digest','old-result');",
    )
    .unwrap();
    drop(old);
    for _ in 0..2 {
        let mut db = Sqlite::open(&path, false).unwrap();
        let rows=db.read_batch(&[
            sql("SELECT schema_version,revision,usage_revision FROM meta"),
            sql("SELECT m.id,m.record_id,m.source,m.created FROM memories m JOIN evidence_index ON evidence_index.rowid=m.rowid WHERE evidence_index MATCH 'words:paper'"),
            receipt("old-request")
        ]).unwrap();
        assert_eq!(
            rows[0][0],
            [Value::Integer(3), Value::Integer(5), Value::Integer(7)]
        );
        assert_eq!(
            rows[1][0],
            [
                Value::Text("summary".into()),
                Value::Text("summary".into()),
                Value::Text("original".into()),
                Value::Integer(13)
            ]
        );
        assert_eq!(rows[2][0][1], Value::Text("old-result".into()));
    }
}

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
fn process_worker() {
    let Ok(path) = std::env::var("OMOIDE_TEST_DIR") else {
        return;
    };
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
            .args(["--exact", "tests::process_worker"])
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

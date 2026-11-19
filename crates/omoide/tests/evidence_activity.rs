use omoide_store::{Rows, Sqlite, Statement, Store, Value as Cell};
use serde_json::{Value, json};
use std::{path::PathBuf, process::Command};
struct Db {
    _dir: tempfile::TempDir,
    path: PathBuf,
}
impl Db {
    fn new() -> Self {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test.db");
        let d = Self { _dir: dir, path };
        d.ok(&["init"]);
        d
    }
    fn run(&self, args: &[&str], ok: bool) -> Value {
        let out = Command::new(env!("CARGO_BIN_EXE_omoide"))
            .arg("--db")
            .arg(&self.path)
            .args(args)
            .output()
            .unwrap();
        let v = serde_json::from_slice(&out.stdout).unwrap();
        assert_eq!(out.status.success(), ok, "{args:?}: {v}");
        v
    }
    fn ok(&self, args: &[&str]) -> Value {
        self.run(args, true)
    }
    fn sql(&self, sql: &str) -> Rows {
        Sqlite::open(&self.path, false)
            .unwrap()
            .read_batch(&[Statement::new(sql, vec![])])
            .unwrap()
            .remove(0)
    }
    fn source(&self) -> String {
        self.ok(&["say", "--text", "An invented source"])["id"]
            .as_str()
            .unwrap()
            .into()
    }
}
fn id(v: &Value) -> &str {
    v["id"].as_str().unwrap()
}
fn version(v: &Value) -> &str {
    v["version_id"].as_str().unwrap()
}

// [REQ-MEASURE-1]
#[test]
fn distinguish_return_use_action_outcome_correction_and_opportunities() {
    let d = Db::new();
    let src = d.source();
    let rule = d.ok(&[
        "rule",
        "add",
        "--source",
        &src,
        "--scene",
        "sorting",
        "--text",
        "Sort paper",
        "--action",
        "sort",
        "--action",
        "file",
        "--target",
        "press",
    ]);
    let v = version(&rule);
    let stats = d.ok(&["stats"]);
    assert_eq!(stats["rules"][0]["measurement"], "not_measured");
    let query = [
        "recall",
        "--action",
        "sort",
        "--target",
        "press",
        "--scene",
        "private scene",
        "--opportunity-id",
        "chance",
        "--call-id",
        "lookup",
    ];
    let reply = d.ok(&query);
    assert_eq!(reply["accounting"], "recorded");
    assert_eq!(reply["results"][0]["used_count"], 0);
    let log = d.sql(
        "SELECT bytes,records,opportunity,query,elapsed_us,created FROM calls WHERE id='lookup'",
    );
    assert_eq!(log[0][0], Cell::Integer(reply.to_string().len() as i64 + 1));
    let Cell::Text(ids) = &log[0][1] else {
        panic!()
    };
    assert_eq!(
        serde_json::from_str::<Value>(ids).unwrap(),
        json!([{"id":id(&rule),"version_id":v}])
    );
    assert_eq!(log[0][2], Cell::Text("chance".into()));
    assert_eq!(log[0][3], Cell::Null);
    assert!(matches!(log[0][4],Cell::Integer(n) if n>=0));
    assert!(matches!(log[0][5],Cell::Integer(n) if n>1));
    let logged = d.ok(&[
        "recall",
        "--scene",
        "public scene",
        "--log-query",
        "--call-id",
        "logged",
    ]);
    assert_eq!(logged["call_id"], "logged");
    assert_eq!(
        d.sql("SELECT query FROM calls WHERE id='logged'")[0][0],
        Cell::Text("public scene".into())
    );
    let used = [
        "used",
        "--version",
        v,
        "--notice-id",
        "n1",
        "--from-call",
        "lookup",
    ];
    let first = d.ok(&used);
    assert_eq!(d.ok(&used), first);
    d.run(
        &[
            "used",
            "--version",
            v,
            "--notice-id",
            "n1",
            "--opportunity-id",
            "different",
        ],
        false,
    );
    d.run(
        &[
            "used",
            "--version",
            v,
            "--notice-id",
            "wrong",
            "--from-call",
            "lookup",
            "--opportunity-id",
            "another",
        ],
        false,
    );
    d.run(&["used", "--version", v, "--from-call", "logged"], false);
    d.run(&["acted", "--version", v], false);
    d.run(&["acted", "--version", v, "--action", "unknown"], false);
    d.ok(&[
        "acted",
        "--version",
        v,
        "--action",
        "sort",
        "--opportunity-id",
        "chance",
    ]);
    d.ok(&["outcome", "--version", v, "--result", "failure"]);
    assert_eq!(d.ok(&["stats"])["rules"][0]["corrections"], 0);
    for (notice, chance) in [("c1", "chance"), ("c2", "chance"), ("c3", "another")] {
        d.ok(&[
            "corrected",
            "--version",
            v,
            "--notice-id",
            notice,
            "--opportunity-id",
            chance,
        ]);
    }
    // This opportunity exists only as an acted notice. The used-only opportunity is not an action.
    d.ok(&[
        "acted",
        "--version",
        v,
        "--action",
        "sort",
        "--opportunity-id",
        "action-only",
    ]);
    d.ok(&["used", "--version", v, "--opportunity-id", "used-only"]);
    let stats = d.ok(&["stats"]);
    let r = &stats["rules"][0];
    assert_eq!(r["opportunities"], 2);
    assert_eq!(r["corrections"], 2);
    assert_eq!(r["measurement"], "measured");
    assert_eq!(stats["check_candidates"], json!([id(&rule)]));
    assert_eq!(
        d.ok(&["stats", "--correction-threshold", "3"])["check_candidates"],
        json!([])
    );
    assert_eq!(
        d.sql("SELECT usage_revision FROM meta")[0][0],
        Cell::Integer(8)
    );
    assert_eq!(
        d.sql("SELECT outcome FROM notices WHERE kind='outcome'")[0][0],
        Cell::Text("failure".into())
    );
    let after = d.ok(&query);
    assert_eq!(after["accounting"], "unlogged");
    assert_eq!(after["results"][0]["used_count"], 2);
}

// [REQ-DEC-1]
#[test]
fn verification_requires_explicit_observation_and_does_not_promote_reports() {
    let d = Db::new();
    let src = d.source();
    let mut fact = vec![
        "fact",
        "add",
        "--source",
        &src,
        "--text",
        "check",
        "--certainty",
        "self-verified",
        "--claim",
        "usable",
        "--target",
        "tray",
    ];
    for (key, val) in [
        ("--verified-where", "bench"),
        ("--verified-when", "55"),
        ("--verified-how", "inspected"),
        ("--evidence", "report-ref"),
    ] {
        d.run(&fact, false);
        fact.extend([key, val]);
    }
    for missing in [
        "--verified-where",
        "--verified-when",
        "--verified-how",
        "--evidence",
    ] {
        let mut partial = fact.clone();
        let idx = partial.iter().position(|s| *s == missing).unwrap();
        partial.drain(idx..idx + 2);
        d.run(&partial, false);
    }
    let value = d.ok(&fact);
    let shown = d.ok(&["show", id(&value)]);
    assert_eq!(shown["record"]["verified_when"], 55);
    let requested = d.ok(&[
        "fact",
        "add",
        "--source",
        &src,
        "--text",
        "delivery requested",
        "--target",
        "tray",
        "--claim",
        "requested",
    ]);
    assert_eq!(
        d.ok(&["show", id(&requested)])["record"]["certainty"],
        "unknown"
    );
    let q = |claim| {
        d.ok(&[
            "recall", "--scene", "check", "--target", "tray", "--kind", "fact", "--claim", claim,
        ])
    };
    assert_eq!(q("usable")["verified"], true);
    let delivery = q("delivered");
    assert_eq!(delivery["verified"], false);
    assert_eq!(delivery["results"], json!([]));
    let request = q("requested");
    assert_eq!(request["verified"], false);
    assert_eq!(request["results"][0]["id"], id(&requested));
    let event = d.ok(&[
        "event",
        "add",
        "--text",
        "opened package",
        "--target",
        "desk",
        "--at",
        "77",
    ]);
    let observed = d.ok(&[
        "fact",
        "add",
        "--source",
        id(&event),
        "--text",
        "delivered",
        "--certainty",
        "verified",
        "--claim",
        "delivered",
    ]);
    let r = d.ok(&["show", id(&observed)]);
    assert_eq!(r["record"]["verified_where"], "desk");
    assert_eq!(r["record"]["verified_how"], "opened package");
    assert_eq!(r["record"]["evidence"], id(&event));
}

// [REQ-DEC-1]
#[test]
fn decision_state_is_independent_of_speech_and_old_decisions_are_not_active() {
    let d = Db::new();
    let src = d.source();
    let other = d.ok(&["say", "--text", "heard elsewhere", "--speaker", "agent"]);
    let decide = |s, state, speech| {
        vec![
            "decide", "--source", s, "--state", state, "--speech", speech, "--text", "packing",
            "--target", "shop",
        ]
    };
    d.run(&decide(id(&other), "decided", "instruction"), false);
    let old = d.ok(&decide(&src, "revised", "instruction"));
    let proposal = d.ok(&decide(&src, "pending", "proposal"));
    let owner = d.ok(&decide(&src, "decided", "instruction"));
    let r = d.ok(&["recall", "--scene", "packing", "--kind", "decision"]);
    let items = r["results"].as_array().unwrap();
    assert_eq!(items.len(), 2);
    assert_eq!(items[0]["id"], id(&owner));
    assert_eq!(items[1]["id"], id(&proposal));
    assert_eq!(items[1]["decision"], "pending");
    assert_eq!(items[1]["speech"], "proposal");
    assert_eq!(items[0]["source_text"], "An invented source");
    assert_eq!(d.ok(&["show", id(&old)])["record"]["decision"], "revised");
}

// [REQ-PROMO-1]
#[test]
fn reported_use_only_changes_order_within_applicable_records() {
    let d = Db::new();
    let src = d.source();
    let base = d.ok(&[
        "fact",
        "add",
        "--source",
        &src,
        "--text",
        "amber paper",
        "--target",
        "shed",
        "--at",
        "1",
    ]);
    let recent = d.ok(&[
        "fact",
        "add",
        "--source",
        &src,
        "--text",
        "amber paper",
        "--target",
        "shed",
        "--at",
        "2",
    ]);
    let observed = d.ok(&["event", "add", "--text", "saw foil", "--target", "shed"]);
    let high = d.ok(&[
        "fact",
        "add",
        "--source",
        id(&observed),
        "--text",
        "silver foil",
        "--target",
        "shed",
        "--certainty",
        "verified",
    ]);
    let query = || {
        d.ok(&[
            "recall",
            "--kind",
            "fact",
            "--scene",
            "amber paper",
            "--target",
            "shed",
        ])
    };
    let first = query();
    assert_eq!(first["results"][0]["id"], id(&recent));
    assert_eq!(first["results"][2]["id"], id(&high));
    assert_eq!(first["results"][2]["match_basis"], "target_only");
    assert_eq!(first["results"][0]["match_basis"], "lexical");
    d.ok(&["used", "--version", version(&base), "--notice-id", "use"]);
    let after = query();
    assert_eq!(after["results"][0]["id"], id(&base));
    assert_eq!(after["results"][0]["used_count"], 1);
    assert_eq!(
        d.ok(&["recall", "--scene", "amber", "--target", "hall"])["matched_count"],
        0
    );
    assert_eq!(
        d.ok(&["recall", "--scene", "dust", "--kind", "fact"])["matched_count"],
        0
    );
    let targeted = d.ok(&[
        "recall", "--scene", "dust", "--target", "shed", "--kind", "fact",
    ]);
    assert_eq!(targeted["results"][0]["id"], id(&high));
    assert_eq!(targeted["results"][0]["match_basis"], "target_only");
}

// [REQ-STORE-1]
#[test]
fn operation_limits_and_notice_identity() {
    let d = Db::new();
    let exact = "a".repeat(4096);
    assert_eq!(
        d.ok(&["say", "--text", "hi", "--operation-id", &exact])["operation_id"],
        exact
    );
    d.run(
        &["say", "--text", "hi", "--operation-id", &"a".repeat(4097)],
        false,
    );
    d.run(&["say", "--text", "  "], false);
    let ev = d.ok(&["event", "add", "--text", "tested"]);
    let args = [
        "used",
        "--version",
        version(&ev),
        "--notice-id",
        "u",
        "--operation-id",
        "u",
    ];
    assert_eq!(d.ok(&args)["notice_id"], "u");
    d.run(
        &[
            "used",
            "--version",
            version(&ev),
            "--notice-id",
            "x",
            "--operation-id",
            "y",
        ],
        false,
    );
    d.run(&["acted", "--version", version(&ev)], false);
    let action = d.ok(&["acted", "--version", version(&ev), "--action", "read"]);
    assert_eq!(action["kind"], "acted");
    assert_eq!(
        d.sql("SELECT action FROM notices WHERE kind='acted'")[0][0],
        Cell::Text("read".into())
    );
}

// [REQ-RECALL-1]
#[test]
fn independent_candidate_caps_and_structured_filters() {
    let d = Db::new();
    let src = d.source();
    let only = |kind| {
        d.ok(&[
            "recall",
            "--scene",
            "paper",
            "--kind",
            kind,
            "--scan-limit",
            "0",
            "--fts-limit",
            "2",
        ])
    };
    for (i, scene) in ["one", "two", "three"].into_iter().enumerate() {
        d.ok(&[
            "rule", "add", "--source", &src, "--text", "paper", "--scene", scene,
        ]);
        let r = only("rule");
        assert_eq!(r["candidate_count"], (i + 1).min(2));
        assert_eq!(r["candidates_truncated"], i >= 2);
        let fact = only("fact");
        assert_eq!(fact["candidate_count"], 0);
        assert_eq!(fact["candidates_truncated"], false);
    }
    for i in 1..=3 {
        d.ok(&[
            "fact", "add", "--source", &src, "--text", "paper", "--scope", "sheet", "--period",
            "winter", "--claim", "entered",
        ]);
        let fact = only("fact");
        assert_eq!(fact["candidate_count"], i.min(2));
        assert_eq!(fact["candidates_truncated"], i > 2);
    }
    for (flag, value) in [
        ("--scope", "sheet"),
        ("--period", "winter"),
        ("--claim", "entered"),
    ] {
        let r = d.ok(&["recall", "--scene", "paper", flag, value]);
        assert_eq!(r["matched_count"], 3);
        assert!(
            r["results"]
                .as_array()
                .unwrap()
                .iter()
                .all(|r| r["kind"] == "fact")
        );
    }
    d.run(&["recall", "--scene", " ", "--action", " "], false);
    d.run(&["recall", "--scene", "paper", "--fts-limit", "0"], false);
}

// [REQ-RECALL-1]
#[test]
fn partial_proof_requires_verified_scope_and_exact_period() {
    let d = Db::new();
    let src = d.source();
    let e = d.ok(&[
        "event",
        "add",
        "--text",
        "observed counted plates",
        "--target",
        "shelf",
        "--at",
        "70",
    ]);
    let f = d.ok(&[
        "fact",
        "add",
        "--text",
        "plates",
        "--source",
        id(&e),
        "--certainty",
        "verified",
        "--period",
        "spring",
        "--scope",
        "round",
        "--claim",
        "counted",
        "--target",
        "shelf",
    ]);
    d.ok(&[
        "fact",
        "add",
        "--text",
        "plates",
        "--source",
        &src,
        "--period",
        "spring",
        "--scope",
        "oval",
        "--claim",
        "counted",
        "--evidence",
        "someone-said",
        "--target",
        "shelf",
    ]);
    let q = |period| {
        d.ok(&[
            "recall", "--scene", "plates", "--period", period, "--scope", "round", "--scope",
            "oval", "--claim", "counted", "--target", "shelf",
        ])
    };
    let r = q("spring");
    assert_eq!(r["unproven_scope"], json!(["oval"]));
    assert_eq!(r["verified"], false);
    assert_eq!(r["results"][0]["id"], id(&f));
    assert_eq!(r["results"][0]["certainty"], "verified");
    assert_eq!(q("summer")["matched_count"], 0);
    // Scoped evidence about this exact claim is verified when its whole requested scope is known.
    assert_eq!(
        d.ok(&[
            "recall", "--scene", "plates", "--claim", "counted", "--scope", "round", "--period",
            "spring", "--target", "shelf"
        ])["verified"],
        true
    );
}

// [REQ-PROMO-1]
#[test]
fn feedback_on_prior_versions_and_pending_only_actions_preserves_applicability() {
    let d = Db::new();
    let init = d.ok(&["init", "--operation-id", "second-init"]);
    assert_eq!(init["store_id"].as_str().unwrap().len(), 36);
    assert_eq!(init, d.ok(&["init", "--operation-id", "second-init"]));
    let s1 = d.source();
    let s2 = d.ok(&["say", "--text", "opposing source"]);
    let base = d.ok(&[
        "rule",
        "add",
        "--source",
        &s1,
        "--text",
        "cobalt stock",
        "--scene",
        "workshop",
        "--action",
        "store",
    ]);
    let mut draft = vec![
        "rule",
        "add",
        "--source",
        id(&s2),
        "--text",
        "cobalt stock",
        "--scene",
        "workshop",
        "--action",
        "supply",
    ];
    let near = d.ok(&draft);
    draft.extend([
        "--relation",
        "unsure",
        "--against",
        version(&base),
        "--near-digest",
        near["near_digest"].as_str().unwrap(),
    ]);
    let pending = d.ok(&draft);
    let by_action = d.ok(&["recall", "--action", "supply"]);
    assert_eq!(
        by_action["results"][0]["unresolved_conflicts"][0]["pending"]["source"]["id"],
        id(&s2)
    );
    let shown = d.ok(&["show", version(&pending)]);
    assert_eq!(
        shown["unresolved_conflicts"][0]["current"]["record"]["version_id"],
        version(&base)
    );
    assert_eq!(
        shown["unresolved_conflicts"][0]["pending"]["record"]["version_id"],
        version(&pending)
    );
    d.ok(&["resolve", version(&pending), "--keep", "pending"]);
    d.ok(&["used", "--version", version(&base)]);
    assert_eq!(d.ok(&["recall", "--action", "store"])["state"], "none");
    let current = d.ok(&["recall", "--action", "supply"]);
    assert_eq!(current["results"][0]["used_count"], 0);
    assert_eq!(current["results"][0]["version_id"], version(&pending));
}

// [REQ-RECALL-1]
#[test]
fn rule_and_evidence_fts_limits_are_independent() {
    let d = Db::new();
    let src = d.source();
    for label in ["one", "two", "three"] {
        d.ok(&[
            "rule", "add", "--source", &src, "--text", "paper", "--scene", label,
        ]);
    }
    let recall = |kind| {
        d.ok(&[
            "recall",
            "--scene",
            "paper",
            "--kind",
            kind,
            "--scan-limit",
            "0",
            "--fts-limit",
            "2",
        ])
    };
    assert_eq!(recall("rule")["candidates_truncated"], true);
    assert_eq!(recall("fact")["candidates_truncated"], false);
    for _ in 0..3 {
        d.ok(&["fact", "add", "--source", &src, "--text", "paper"]);
    }
    let all = d.ok(&[
        "recall",
        "--scene",
        "paper",
        "--scan-limit",
        "0",
        "--fts-limit",
        "2",
    ]);
    assert_eq!(all["matched_count"], 4);
    let evidence = d.ok(&[
        "recall",
        "--scene",
        "paper",
        "--kind",
        "fact",
        "--fts-limit",
        "2",
    ]);
    assert_eq!(evidence["results"][0]["match_basis"], "lexical");
    assert!(
        all["results"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|r| r["kind"] == "fact")
            .all(|r| r.get("purpose").is_none())
    );
}

// [REQ-DEC-1]
#[test]
fn uncertain_target_or_period_does_not_turn_scoped_proof_into_global_confirmation() {
    let d = Db::new();
    let event = d.ok(&[
        "event",
        "add",
        "--text",
        "inspected tray",
        "--target",
        "depot",
    ]);
    let fact = d.ok(&[
        "fact",
        "add",
        "--source",
        id(&event),
        "--text",
        "received",
        "--target",
        "depot",
        "--certainty",
        "verified",
        "--claim",
        "delivered",
        "--period",
        "spring",
        "--scope",
        "tray",
    ]);
    let common = [
        "recall",
        "--scene",
        "received",
        "--kind",
        "fact",
        "--claim",
        "delivered",
        "--scope",
        "tray",
    ];
    for selector in [["--target", "depot"], ["--period", "spring"]] {
        let mut args = common.to_vec();
        args.extend(selector);
        let response = d.ok(&args);
        assert_eq!(response["results"][0]["id"], id(&fact));
        assert_eq!(response["verified"], false);
        assert_eq!(response["unproven_scope"], json!(["tray"]));
    }
}

// [REQ-DEC-1]
#[test]
fn revised_decision_keeps_both_sources_and_usage_cannot_revive_old_consent() {
    let d = Db::new();
    let src = d.source();
    let base = d.ok(&[
        "decide",
        "--source",
        &src,
        "--text",
        "ship amber",
        "--state",
        "decided",
        "--speech",
        "instruction",
    ]);
    let changed = d.ok(&["say", "--text", "reconsider shipment"]);
    let cmd = [
        "decide",
        "--source",
        id(&changed),
        "--text",
        "hold violet",
        "--state",
        "pending",
        "--speech",
        "proposal",
        "--revises",
        version(&base),
        "--operation-id",
        "reconsider",
    ];
    let next = d.ok(&cmd);
    assert_eq!(next, d.ok(&cmd));
    assert_eq!(id(&next), id(&base));
    assert_ne!(version(&next), version(&base));
    // A new operation may not apply an old choice to a new version.
    let mut stale = cmd;
    *stale.last_mut().unwrap() = "stale";
    d.run(&stale, false);
    d.ok(&["used", "--version", version(&base)]);
    let missing = d.ok(&["recall", "--scene", "amber"]);
    assert_eq!(missing["state"], "incomplete_search");
    assert!(missing["results"].as_array().unwrap().is_empty());
    let h = d.ok(&["show", id(&base), "--history"]);
    assert_eq!(h["record"]["decision"], "pending");
    assert_eq!(h["history"][0]["record"]["decision"], "revised");
    assert_eq!(h["history"][0]["record"]["successor"], version(&next));
    assert_eq!(h["history"][0]["source"]["id"], src);
    assert_eq!(h["history"][1]["source"]["id"], id(&changed));
    assert_eq!(
        d.ok(&["show", version(&base)])["record"]["decision"],
        "revised"
    );
}

// [REQ-DEC-1]
#[test]
fn a_reported_fact_with_evidence_is_still_unproven_for_its_own_scope() {
    let d = Db::new();
    let source = d.source();
    let fact = d.ok(&[
        "fact",
        "add",
        "--source",
        &source,
        "--text",
        "plate count",
        "--certainty",
        "reported",
        "--target",
        "kiln",
        "--scope",
        "round",
        "--period",
        "spring",
        "--claim",
        "counted",
        "--evidence",
        "inventory-note",
    ]);
    let reply = d.ok(&[
        "recall",
        "--kind",
        "fact",
        "--scene",
        "plate count",
        "--target",
        "kiln",
        "--scope",
        "round",
        "--period",
        "spring",
        "--claim",
        "counted",
    ]);
    let records = reply["results"].as_array().unwrap();
    assert_eq!(records.len(), 1);
    assert_eq!(records[0]["id"], id(&fact));
    assert_eq!(records[0]["match_basis"], "lexical");
    assert_eq!(records[0]["certainty"], "reported");
    assert_eq!(records[0]["evidence"], "inventory-note");
    assert_eq!(reply["verified"], false);
    assert_eq!(reply["unproven_scope"], json!(["round"]));
}

// [REQ-DEC-1]
#[test]
fn decided_and_verified_records_outrank_newer_reports_but_pending_decisions_do_not() {
    let d = Db::new();
    let source = d.source();
    // Equal lexical scores and usage. Both priority records are older than both ordinary records.
    let decide = |state, at| {
        d.ok(&[
            "decide",
            "--source",
            &source,
            "--text",
            "binding",
            "--target",
            "workshop",
            "--speech",
            "instruction",
            "--state",
            state,
            "--at",
            at,
        ])
    };
    let decided = decide("decided", "1");
    let pending = decide("pending", "2");
    let fact = d.ok(&[
        "fact",
        "add",
        "--source",
        &source,
        "--text",
        "binding",
        "--target",
        "workshop",
        "--certainty",
        "reported",
        "--at",
        "3",
    ]);
    let verified = d.ok(&[
        "fact",
        "add",
        "--source",
        &source,
        "--text",
        "binding",
        "--target",
        "workshop",
        "--certainty",
        "verified",
        "--verified-where",
        "bench",
        "--verified-when",
        "0",
        "--verified-how",
        "inspected",
        "--evidence",
        "inspection-note",
        "--at",
        "0",
    ]);
    let reply = d.ok(&["recall", "--scene", "binding", "--target", "workshop"]);
    let records = reply["results"].as_array().unwrap();
    assert!(records.iter().all(|r| r["match_basis"] == "lexical"));
    assert_eq!(
        records.iter().map(id).collect::<Vec<_>>(),
        [id(&decided), id(&verified), id(&fact), id(&pending)]
    );
}

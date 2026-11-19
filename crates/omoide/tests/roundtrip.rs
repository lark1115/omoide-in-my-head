use serde_json::Value;
use std::{path::Path, process::Command};
use tempfile::TempDir;

fn invoke(path: &Path, args: &[&str], success: bool) -> Value {
    let output = Command::new(env!("CARGO_BIN_EXE_omoide"))
        .arg("--db")
        .arg(path)
        .args(args)
        .output()
        .unwrap();
    let json: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(output.status.success(), success, "{json}");
    json
}
struct Fixture {
    _dir: TempDir,
    path: std::path::PathBuf,
    source: String,
}
impl Fixture {
    fn new(text: &str) -> Self {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("roundtrip.db");
        invoke(&path, &["init"], true);
        let s = invoke(
            &path,
            &["say", "--text", text, "--target", "workshop"],
            true,
        );
        Self {
            _dir: dir,
            path,
            source: s["id"].as_str().unwrap().into(),
        }
    }
    fn add(&self, text: &str, scene: &str, extra: &[&str]) -> Value {
        let mut args = vec![
            "rule",
            "add",
            "--source",
            &self.source,
            "--text",
            text,
            "--scene",
            scene,
        ];
        args.extend_from_slice(extra);
        invoke(&self.path, &args, true)
    }
    fn recall(&self, args: &[&str]) -> Value {
        let mut cmd = vec!["recall"];
        cmd.extend_from_slice(args);
        invoke(&self.path, &cmd, true)
    }
}
// [REQ-INTEG-1]
#[test]
fn fresh_process_round_trip_preserves_source() {
    let source = "  Check the latch.\nKeep the rear hatch closed.  ";
    let f = Fixture::new(source);
    let r = f.add(
        "Inspect the latch",
        "close hatch",
        &[
            "--action",
            "seal",
            "--alias",
            "格納庫の扉",
            "--target",
            "workshop",
            "--purpose",
            "keep dust out",
            "--exclude",
            "training",
        ],
    );
    let id = r["id"].as_str().unwrap();
    assert_eq!(r["state"], "activated");
    for query in [
        vec!["--scene", "格納庫", "--target", "workshop"],
        vec!["--action", "seal", "--target", "workshop"],
    ] {
        let recall = f.recall(&query);
        assert_eq!(recall["state"], "found");
        let record = &recall["results"][0];
        assert_eq!(record["id"], id);
        assert_eq!(record["kind"], "rule");
        assert_eq!(record["state"], "current");
        assert_eq!(record["purpose"], "keep dust out");
        assert_eq!(record["exclusion"], "training");
        assert_eq!(record["source_id"], f.source);
    }
    assert_eq!(
        f.recall(&["--action", "seal"])["results"][0]["target"],
        "workshop"
    );
    for args in [
        vec!["--action", "seal", "--target", "annex"],
        vec!["--action", "open", "--target", "workshop"],
    ] {
        assert_eq!(f.recall(&args)["state"], "none");
    }
    let shown = invoke(&f.path, &["show", id], true);
    assert_eq!(shown["source"]["text"], source);
    assert_eq!(shown["source"]["target"], "workshop");
    assert!(shown["source"]["created"].is_number());
    assert_eq!(
        invoke(&f.path, &["show", &f.source], true)["record"]["text"],
        source
    );
    assert_eq!(
        invoke(&f.path, &["show", "missing"], false)["state"],
        "error"
    );
}
// [REQ-RULE-1]
#[test]
fn near_returns_evidence_without_writing() {
    let f = Fixture::new("Inspect all seals.");
    let add = f.add(
        "鉛筆 倉庫",
        "棚卸",
        &["--action", "inventory", "--operation-id", "first"],
    );
    assert_eq!(
        add,
        f.add(
            "鉛筆 倉庫",
            "棚卸",
            &["--action", "inventory", "--operation-id", "first"]
        )
    );
    for (text, action) in [("unrelated terms", "inventory"), ("鉛筆 倉庫", "other")] {
        let near = f.add(
            text,
            "different",
            &["--action", action, "--operation-id", "unused"],
        );
        assert_eq!(near["state"], "near_rules");
        assert_eq!(near["near_rules"][0]["record"]["id"], add["id"]);
        assert_eq!(
            near["near_rules"][0]["source"]["text"],
            "Inspect all seals."
        );
        assert_eq!(near["near_digest"].as_str().unwrap().len(), 64);
    }
    // A read-only near result does not consume the operation ID.
    assert_eq!(
        f.add("flux", "solder", &["--operation-id", "unused"])["state"],
        "activated"
    );
    assert_eq!(
        invoke(
            &f.path,
            &["say", "--text", "changed", "--operation-id", "first"],
            false
        )["kind"],
        "invalid_input"
    );
    assert_eq!(
        invoke(
            &f.path,
            &[
                "rule", "add", "--source", "absent", "--text", "flux", "--scene", "solder"
            ],
            false
        )["state"],
        "error"
    );
}
// [REQ-JUDGE-1]
#[test]
fn indexed_and_scan_paths_agree_on_exact_matches_and_exclusions() {
    let f = Fixture::new("Inspect the proof.");
    let a = f.add(
        "校正 ＰＲＯＯＦ",
        "刷版",
        &["--action", "print", "--exclude", "archive"],
    );
    for limit in ["0", "500"] {
        for term in ["版", "校正", "proof"] {
            let r = f.recall(&["--scene", term, "--scan-limit", limit]);
            assert_eq!(r["judge_state"], "not_configured");
            assert_eq!(r["results"][0]["id"], a["id"]);
            assert_eq!(r["results"][0]["purpose"], "unknown");
        }
        for term in ["板", "proofs"] {
            assert_eq!(
                f.recall(&["--scene", term, "--scan-limit", limit])["state"],
                "none"
            );
        }
        assert_eq!(
            f.recall(&[
                "--action",
                "print",
                "--target",
                "archive",
                "--scan-limit",
                limit
            ])["state"],
            "none"
        );
    }
}
// [REQ-RECALL-1]
#[test]
fn output_budget_pages_and_restarts_after_write() {
    let f = Fixture::new("Preserve samples.");
    let large = "保管。".repeat(1200);
    f.add(&large, "amber", &["--action", "a"]);
    f.add("second", "amber", &["--action", "b"]);
    let r = f.recall(&["--action", "b", "--scene", "amber"]);
    assert!(r.to_string().len() < 4000);
    let cursor = r["cursor"].as_str().unwrap();
    let next = f.recall(&["--action", "b", "--scene", "amber", "--cursor", cursor]);
    assert!(next.to_string().len() < 4000);
    assert_eq!(next["results"][0]["truncated"], true);
    assert_ne!(next["results"][0]["id"], r["results"][0]["id"]);
    invoke(&f.path, &["say", "--text", "new event"], true);
    assert_eq!(
        f.recall(&["--action", "b", "--scene", "amber", "--cursor", cursor])["restarted"],
        true
    );
}

// [REQ-REC-1]
#[test]
fn full_source_survives_registration_and_replay() {
    let f = Fixture::new("fixture");
    let text = format!("{}\n  final condition  ", "原文".repeat(2400));
    let args = [
        "say",
        "--operation-id",
        "source-retry",
        "--at",
        "1234",
        "--text",
        &text,
    ];
    let receipt = invoke(&f.path, &args, true);
    assert_eq!(receipt, invoke(&f.path, &args, true));
    let actual = invoke(&f.path, &["show", receipt["id"].as_str().unwrap()], true);
    assert_eq!(actual["record"]["text"], text);
    assert_eq!(actual["record"]["created"], 1234);
}

// [REQ-RES-1]
#[test]
#[ignore = "Linux release benchmark; requires Python 3 and GNU time"]
fn linux_release_resource_budget() {
    let script = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../verify/measure.py");
    assert!(
        Command::new("python3")
            .arg(script)
            .arg(env!("CARGO_BIN_EXE_omoide"))
            .status()
            .unwrap()
            .success()
    );
}

// [REQ-JUDGE-1]
#[test]
fn weak_lexical_match_and_structured_judge_candidates() {
    let f = Fixture::new("Invented workshop instructions");
    let a = f.add(
        "wrap in paper",
        "after drying",
        &["--action", "pack", "--target", "tray"],
    );
    let b = f.add(
        "paint with amber",
        "before opening",
        &["--action", "coat", "--target", "tray"],
    );
    assert_eq!(
        f.recall(&[
            "--scene",
            "amber brass tin jade felt linen cotton oil chalk"
        ])["state"],
        "none"
    );
    let weak = f.recall(&["--scene", "amber brass tin jade felt linen cotton oil"]);
    assert_eq!(weak["results"][0]["id"], b["id"]);
    let path = f.path.with_file_name("judge.py");
    std::fs::write(&path, "import json,sys\nr=json.load(sys.stdin)\nprint(json.dumps({'answers':[{'version_id':c['version_id'],'score':1} for c in r['candidates']]}))\n").unwrap();
    let judge = ["--judge", "python3", "--judge-arg", path.to_str().unwrap()];
    for (query, expected_count) in [
        (vec!["--action", "pack", "--target", "tray"], 1),
        (vec!["--action", "pack"], 2),
        (vec!["--scene", "different", "--target", "tray"], 2),
        (
            vec![
                "--scene",
                "different",
                "--action",
                "pack",
                "--target",
                "tray",
            ],
            2,
        ),
    ] {
        let response = f.recall(&[query.as_slice(), &judge].concat());
        assert_eq!(response["judge_state"], "used");
        let rows = response["results"].as_array().unwrap();
        assert_eq!(rows.len(), expected_count);
        assert!(rows.iter().any(|r| r["id"] == a["id"]));
    }
}

// [REQ-RECALL-1]
#[test]
fn target_and_action_words_do_not_supply_extra_trigger_evidence() {
    let f = Fixture::new("Label workshop supplies");
    let a = f.add("violet copper linen", "violet copper", &[]);
    let b = f.add(
        "spruce ash",
        "spruce ash",
        &["--target", "spruce", "--action", "ash"],
    );
    assert_ne!(a["id"], b["id"]);
    let response = f.recall(&["--scene", "violet copper linen spruce ash"]);
    let ids: Vec<_> = response["results"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| &r["id"])
        .collect();
    assert_eq!(ids, vec![&a["id"]]);
}

// [REQ-RECALL-1]
#[test]
fn strongest_qualifying_rule_survives_its_own_relative_cut() {
    let f = Fixture::new("Packing materials");
    let a = f.add("linen copper brass felt", "linen copper brass", &[]);
    let response = f.recall(&["--scene", "linen copper brass felt"]);
    assert_eq!(response["results"][0]["id"], a["id"]);
    assert_eq!(response["state"], "found");
}

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
    for (text, action) in [("鉛筆 倉庫", "inventory"), ("鉛筆 倉庫", "other")] {
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
                if limit == "0" {
                    "incomplete_search"
                } else {
                    "none"
                }
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
    let second = f.add("second", "amber", &["--action", "b"]);
    assert_eq!(second["state"], "near_rules");
    f.add(
        "second",
        "amber",
        &[
            "--action",
            "b",
            "--relation",
            "unrelated",
            "--against",
            second["near_rules"][0]["record"]["version_id"]
                .as_str()
                .unwrap(),
            "--near-digest",
            second["near_digest"].as_str().unwrap(),
        ],
    );

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
    let tentative = f.recall(&[
        "--scene",
        "amber brass tin jade felt linen cotton oil chalk",
    ]);
    assert_eq!(tentative["state"], "candidates_only");
    assert_eq!(tentative["results"], serde_json::json!([]));
    assert_eq!(tentative["candidates"][0]["version_id"], b["version_id"]);
    assert_eq!(tentative["candidates_remaining"], 0);
    let weak = f.recall(&["--scene", "amber brass tin jade felt linen cotton oil"]);
    assert_eq!(weak["results"][0]["id"], b["id"]);
    let path = f.path.with_file_name("judge.py");
    std::fs::write(&path, "import json,sys\nr=json.load(sys.stdin)\nprint(json.dumps({'answers':[{'version_id':c['version_id'],'score':1} for c in r['candidates']]}))\n").unwrap();
    let judge = ["--judge", "python3", "--judge-arg", path.to_str().unwrap()];
    for (query, expected_count) in [
        (vec!["--action", "pack", "--target", "tray"], 1),
        (vec!["--action", "pack"], 1),
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
            1,
        ),
    ] {
        let response = f.recall(&[query.as_slice(), &judge].concat());
        assert_eq!(response["judge_state"], "used");
        if !query.contains(&"--scene") {
            assert_eq!(response["search"]["by_kind"]["rule"]["eligible"], 1);
            assert_eq!(response["search"]["by_kind"]["rule"]["scored"], 1);
            assert_eq!(response["search"]["by_kind"]["rule"]["judged"], 1);
            assert_eq!(response["search"]["judge_remaining"], 0);
        }
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

// [REQ-JUDGE-1]
#[test]
fn provisional_judge_scores_share_the_page_after_confirmed_rows() {
    let f = Fixture::new("Invented workshop");
    for i in 0..12 {
        let result = f.add(&format!("item{i}"), &format!("scene{i}"), &[]);
        assert!(result["version_id"].is_string());
    }
    let script = f.path.with_file_name("score.py");
    std::fs::write(&script,"import json, sys\nreq=json.load(sys.stdin)\nprint(json.dumps({'answers':[{'version_id':r['version_id'],'score':1 if r['text'] in ('item0','item1','item2') else float(sys.argv[1])} for r in req['candidates']]}))").unwrap();
    let query = |score| {
        f.recall(&[
            "--scene",
            "unfamiliar",
            "--judge",
            "python3",
            "--judge-arg",
            script.to_str().unwrap(),
            "--judge-arg",
            score,
            "--threshold",
            "0.6",
        ])
    };
    let high = query("0.29");
    let both = query("0.30");
    assert_eq!(high["results"], both["results"]);
    assert_eq!(high["results"].as_array().unwrap().len(), 3);
    assert_eq!(high["candidates"], serde_json::json!([]));
    assert_eq!(both["state"], "found");
    assert_eq!(
        both["candidate_guidance"],
        "Not confirmed to apply; read with show before using."
    );
    let short = both["candidates"].as_array().unwrap();
    assert!(!short.is_empty());
    assert_eq!(
        short.len() + both["candidates_remaining"].as_u64().unwrap() as usize,
        9
    );
    assert!(both.to_string().len() < 4000);
    for r in short {
        assert!(r.get("source_id").is_none());
        assert!(
            both["results"]
                .as_array()
                .unwrap()
                .iter()
                .all(|c| c["version_id"] != r["version_id"])
        );
    }
}

// [REQ-RECALL-1]
#[test]
fn coverage_distinguishes_a_complete_empty_scan_from_an_index_miss() {
    let f = Fixture::new("Workshop record");
    f.add(
        "fold linen",
        "closing",
        &["--action", "fold", "--target", "booth"],
    );
    for (limit, expected, path, scored) in [
        ("9", "none", "full_scan", 1),
        ("0", "incomplete_search", "fts", 0),
    ] {
        let r = f.recall(&[
            "--scene",
            "amber",
            "--scan-limit",
            limit,
            "--target",
            "booth",
        ]);
        assert_eq!(r["state"], expected);
        assert_eq!(r["search"]["rule_path"], path);
        assert_eq!(
            r["search"]["by_kind"]["rule"],
            serde_json::json!({"eligible":1,"scored":scored,"judged":0})
        );
        assert_eq!(r["search"]["not_judged"], 1);
        assert_eq!(
            r["notice"],
            "The device did not find a confirmed match. This is not evidence that no record exists. Search the other sources you have."
        );
        assert_eq!(r["search"]["incomplete"], scored == 0);
        assert_eq!(r["search"]["timeout"], false);
        assert_eq!(r["search"]["fallback"], false);
        assert_eq!(r["search"]["reply_cap"], false);
        assert!(r.to_string().len() < 4000);
    }
    let excluded = f.recall(&["--action", "fold", "--target", "annex"]);
    assert_eq!(excluded["state"], "none");
    assert_eq!(excluded["search"]["by_kind"]["rule"]["eligible"], 0);
    let found = f.recall(&["--action", "fold"]);
    assert_eq!(found["state"], "found");
    assert_eq!(found["search"]["action_index"], true);
    assert_eq!(found["search"]["evidence_path"], "not_requested");
    assert_eq!(
        f.recall(&["--scene", "linen", "--kind", "rule"])["search"]["evidence_path"],
        "not_requested"
    );
    assert!(found.get("notice").is_none());
}

// [REQ-JUDGE-1]
#[test]
fn report_size_caps_separately_from_deadlines() {
    let f = Fixture::new("Packing rule");
    let text = format!("Use paper. {}", "a".repeat(25000));
    f.add(&text, "wrapping", &["--action", "wrap"]);
    let r = f.recall(&["--action", "wrap", "--judge", "nonexistent-test-judge"]);
    assert_eq!(r["judge_error"], "judge question exceeds byte budget");
    let flags = &r["search"];
    assert_eq!(flags["judge_oversized"], 1);
    assert_eq!(flags["judge_remaining"], 1);
    assert_eq!(flags["by_kind"]["rule"]["judged"], 0);
    assert_eq!(flags["timeout"], false);
    assert_eq!(flags["fallback"], true);
    assert_eq!(flags["reply_cap"], true);
    assert_eq!(flags["judge_output_cap"], false);
    let fresh = Fixture::new("Folding rule");
    fresh.add("Fold the card", "folding", &["--action", "fold"]);
    let r = fresh.recall(&[
        "--action",
        "fold",
        "--judge",
        "python3",
        "--judge-arg",
        "-c",
        "--judge-arg",
        "print('x'*200)",
        "--output-bytes",
        "16",
        "--timeout-ms",
        "5000",
    ]);
    assert_eq!(r["search"]["judge_output_cap"], true);
    assert_eq!(r["search"]["timeout"], false);
    assert_eq!(r["search"]["judge_oversized"], 0);
    assert_eq!(r["results"].as_array().unwrap().len(), 1);
}

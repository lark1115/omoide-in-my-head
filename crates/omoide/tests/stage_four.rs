use serde_json::{Value, json};
use std::{fs, path::PathBuf, process::Command};

struct Fixture {
    dir: tempfile::TempDir,
}
impl Fixture {
    fn new() -> Self {
        let f = Self {
            dir: tempfile::tempdir().unwrap(),
        };
        f.ok(&["init"]);
        f
    }
    fn path(&self, name: &str) -> PathBuf {
        self.dir.path().join(name)
    }
    fn call(&self, args: &[&str], ok: bool) -> Value {
        let out = Command::new(env!("CARGO_BIN_EXE_omoide"))
            .arg("--db")
            .arg(self.path("test.db"))
            .args(args)
            .output()
            .unwrap();
        let result = serde_json::from_slice(&out.stdout).unwrap();
        assert_eq!(out.status.success(), ok, "{args:?}: {result}");
        result
    }
    fn ok(&self, args: &[&str]) -> Value {
        self.call(args, true)
    }
    fn rule(&self, body: &str, pinned: bool) -> Value {
        let src = self.ok(&["say", "--text", body]);
        let mut args = vec![
            "rule",
            "add",
            "--scene",
            "loading",
            "--action",
            "load",
            "--text",
            body,
            "--source",
            id(&src),
        ];
        if pinned {
            args.push("--pinned");
        }
        self.ok(&args)
    }
    fn event(&self) -> Value {
        self.ok(&[
            "event",
            "add",
            "--text",
            "inspect blue crates",
            "--target",
            "depot",
        ])
    }
    fn propose(&self, body: &str, events: &[&Value]) -> Value {
        let mut args = vec![
            "skill",
            "propose",
            "--scene",
            "loading",
            "--text",
            body,
            "--action",
            "load",
            "--reference",
            "manual/inspection",
            "--target",
            "depot",
        ];
        for e in events {
            args.extend(["--source", id(e)]);
        }
        self.ok(&args)
    }
}
fn id(v: &Value) -> &str {
    v["id"].as_str().unwrap()
}
fn ver(v: &Value) -> &str {
    v["version_id"].as_str().unwrap()
}

// [REQ-PROMO-1]
#[test]
fn experience_to_skill_to_use_to_export_and_optional_demotion() {
    let f = Fixture::new();
    let e = f.event();
    let second = f.event();
    let groups = f.ok(&["repeated"]);
    assert_eq!(groups["groups"][0]["count"], 2);
    assert_eq!(f.ok(&["repeated", "--min", "3"])["groups"], json!([]));
    let draft = f.propose("check the crate", &[&e, &second]);
    assert_eq!(
        f.ok(&["recall", "--kind", "skill", "--action", "load"])["state"],
        "none"
    );
    let confirmed = f.ok(&["skill", "confirm", ver(&draft)]);
    assert_eq!(confirmed["kind"], "skill");
    assert_eq!(confirmed["id"], draft["id"]);
    let shown = f.ok(&["show", ver(&confirmed)]);
    assert_eq!(shown["sources"].as_array().unwrap().len(), 2);
    assert_eq!(shown["source"]["kind"], "experience");
    let peer = f.propose("compare seal", &[&second]);
    let peer = f.ok(&["skill", "confirm", ver(&peer)]);
    let recall = || {
        f.ok(&[
            "recall", "--action", "load", "--target", "depot", "--kind", "skill",
        ])
    };
    assert_eq!(recall()["results"][0]["id"], peer["id"]);
    f.ok(&["used", "--version", ver(&confirmed)]);
    assert_eq!(recall()["results"][0]["id"], confirmed["id"]);
    let file = f.path("instructions");
    let path = file.to_str().unwrap();
    f.ok(&["hot", "init", path]);
    f.ok(&["hot", "export", path]);
    let block = fs::read_to_string(file).unwrap();
    assert!(
        block.contains("loading") && block.contains("load") && block.contains("manual/inspection")
    );
    assert!(block.find("check the crate").unwrap() < block.find("compare seal").unwrap());
    let outcome = |result| f.ok(&["outcome", "--version", ver(&confirmed), "--result", result]);
    outcome("failure");
    outcome("failure");
    assert_eq!(recall()["results"][0]["failure_count"], 2);
    outcome("success");
    assert_eq!(recall()["results"][0]["failure_count"], 2);
    assert_eq!(recall()["results"][0]["consecutive_failures"], 0);
    for n in 1..=2 {
        f.ok(&[
            "outcome",
            "--version",
            ver(&confirmed),
            "--result",
            "failure",
            "--demote-after",
            "2",
        ]);
        assert_eq!(
            f.ok(&["show", ver(&confirmed)])["record"]["consecutive_failures"],
            n
        );
    }
    assert_eq!(
        f.ok(&["show", ver(&confirmed)])["record"]["state"],
        "candidate"
    );
    assert_eq!(recall()["matched_count"], 1);
}

// [REQ-PROMO-1]
#[test]
fn unsure_skill_needs_resolution_and_preserves_both_origins() {
    let f = Fixture::new();
    let rule = f.rule("inspect at loading", false);
    let event = f.event();
    let draft = f.propose("inspect the crate", &[&event]);
    let v = ver(&draft);
    let near = f.ok(&["skill", "confirm", v]);
    assert_eq!(near["state"], "near_rules");
    assert_eq!(
        near["near_rules"][0]["record"]["version_id"],
        rule["version_id"]
    );
    let pending = f.ok(&[
        "skill",
        "confirm",
        v,
        "--relation",
        "unsure",
        "--against",
        ver(&rule),
        "--near-digest",
        near["near_digest"].as_str().unwrap(),
    ]);
    assert_eq!(pending["state"], "pending");
    assert_eq!(
        f.ok(&[
            "recall", "--kind", "skill", "--action", "load", "--target", "depot"
        ])["matched_count"],
        0
    );
    let pair = &pending["unresolved_conflicts"][0];
    assert_eq!(pair["current"]["source"]["kind"], "utterance");
    assert_eq!(pair["pending"]["source"]["kind"], "experience");
    assert_eq!(pair["pending"]["record"]["kind"], "skill");
    let r = f.ok(&["recall", "--action", "load", "--target", "depot"]);
    assert_eq!(r["results"][0]["kind"], "rule");
    assert_eq!(
        r["results"][0]["unresolved_conflicts"][0]["pending"]["record"]["kind"],
        "skill"
    );
    assert_eq!(
        f.ok(&["resolve", ver(&pending), "--keep", "pending"])["kind"],
        "skill"
    );
    let r = f.ok(&["recall", "--action", "load", "--target", "depot"]);
    assert_eq!(r["results"][0]["kind"], "skill");
    assert_eq!(r["results"][0]["reference"], "manual/inspection");
}

// [REQ-HOT-1]
#[test]
fn section_is_bounded_and_preserves_surrounding_bytes() {
    let f = Fixture::new();
    let file = f.path("instructions");
    let path = file.to_str().unwrap();
    let original = "outside\r\n";
    fs::write(&file, original).unwrap();
    f.call(&["hot", "export", path], false);
    assert_eq!(fs::read_to_string(&file).unwrap(), original);
    f.ok(&["hot", "init", path]);
    let rule = f.rule("Keep the lid closed", true);
    assert_eq!(f.ok(&["hot", "status", path])["state"], "stale");
    let before = fs::read_to_string(&file).unwrap() + "tail\r\n";
    fs::write(&file, &before).unwrap();
    let err = f.call(&["hot", "export", path, "--limit-bytes", "60"], false);
    assert!(err["message"].as_str().unwrap().contains("exceeds cap by"));
    assert_eq!(fs::read_to_string(&file).unwrap(), before);
    let r = f.ok(&["hot", "export", path, "--limit-bytes", "768"]);
    assert!(r["section_bytes"].as_u64().unwrap() <= 768);
    let after = fs::read_to_string(&file).unwrap();
    assert!(
        after.starts_with(original)
            && after.ends_with("tail\r\n")
            && after.contains("Keep the lid closed")
    );
    assert_eq!(
        f.ok(&["hot", "status", path, "--limit-bytes", "768"])["state"],
        "current"
    );
    assert_eq!(f.ok(&["hot", "status", path])["state"], "stale");
    f.ok(&["used", "--version", ver(&rule)]);
    assert_eq!(
        f.ok(&["hot", "status", path, "--limit-bytes", "768"])["state"],
        "stale"
    );
    for broken in [
        after.replace("<!-- omoide:end -->", ""),
        after.replace("<!-- omoide:begin -->", "<!-- omoide:broken -->"),
        after.clone() + &after,
    ] {
        fs::write(&file, &broken).unwrap();
        f.call(&["hot", "export", path], false);
        assert_eq!(fs::read_to_string(&file).unwrap(), broken);
    }
}

// [REQ-JUDGE-1]
#[test]
fn judge_failures_return_lexical_results_and_empty_is_a_success() {
    let f = Fixture::new();
    let rule = f.rule("blue paper", false);
    let script = f.path("judge.py");
    let call = |code: &str, scene: &str| {
        fs::write(&script, code).unwrap();
        f.ok(&[
            "recall",
            "--scene",
            scene,
            "--judge",
            "python3",
            "--judge-arg",
            script.to_str().unwrap(),
            "--timeout-ms",
            "350",
            "--output-bytes",
            "1024",
        ])
    };
    for code in [
        "print('not json')",
        "print('x'*1025)",
        "import time; time.sleep(10)",
        "raise SystemExit(3)",
        "print('{\u{22}answers\u{22}:[{\u{22}version_id\u{22}:\u{22}unknown\u{22},\u{22}score\u{22}:1}]}')",
    ] {
        let r = call(code, "paper");
        assert_eq!(r["judge_state"], "failed");
        assert_eq!(r["results"][0]["id"], rule["id"]);
    }
    let r = call("print('{\u{22}answers\u{22}:[]}')", "paper");
    assert_eq!(r["judge_state"], "used");
    assert_eq!(r["matched_count"], 0);
    let r = call(
        "import sys,json; q=json.load(sys.stdin); print(json.dumps({'answers':[{'version_id':c['version_id'],'score':1} for c in q['candidates']]}))",
        "unrelated",
    );
    assert_eq!(r["judge_state"], "used");
    assert_eq!(r["results"][0]["id"], rule["id"]);
}

// [REQ-HOT-1]
#[test]
fn cap_includes_markers_exactly_and_optional_rows_stay_searchable() {
    let f = Fixture::new();
    let path = f.path("notes");
    let name = path.to_str().unwrap();
    let pinned = f.rule("必須の確認", true);
    let init = f.ok(&["hot", "init", name, "--limit-bytes", "800"]);
    let bytes = init["section_bytes"].as_u64().unwrap();
    let exact = bytes.to_string();
    let less = (bytes - 1).to_string();
    assert_eq!(
        f.ok(&["hot", "export", name, "--limit-bytes", &exact])["section_bytes"],
        bytes
    );
    let previous = fs::read(&path).unwrap();
    let error = f.call(&["hot", "export", name, "--limit-bytes", &less], false);
    assert_eq!(
        error["message"],
        "invalid input: section exceeds cap by 1 bytes"
    );
    assert_eq!(fs::read(&path).unwrap(), previous);
    let src = f.ok(&["say", "--text", "optional"]);
    f.ok(&[
        "rule",
        "add",
        "--source",
        id(&src),
        "--text",
        "optional instruction",
        "--scene",
        "fold paper",
        "--action",
        "fold",
    ]);
    f.ok(&["hot", "export", name, "--limit-bytes", &exact]);
    let saved = fs::read_to_string(&path).unwrap();
    assert!(saved.contains("必須の確認"));
    assert!(!saved.contains("optional instruction"));
    assert_eq!(f.ok(&["recall", "--action", "fold"])["matched_count"], 1);
    // Equal-length revision and usage digits keep the section boundary stable.
    assert_eq!(f.ok(&["show", ver(&pinned)])["record"]["pinned"], 1);
}

// [REQ-PROMO-1]
#[test]
fn skill_relations_keep_experience_sources_and_reject_stale_choices() {
    for relation in ["same", "restated", "exception", "unrelated"] {
        let f = Fixture::new();
        let base = f.rule("rule text", false);
        let a = f.event();
        let b = f.event();
        let draft = f.propose("skill text", &[&a, &b]);
        let near = f.ok(&["skill", "confirm", ver(&draft)]);
        let confirm = |digest: &str, ok| {
            f.call(
                &[
                    "skill",
                    "confirm",
                    ver(&draft),
                    "--against",
                    ver(&base),
                    "--relation",
                    relation,
                    "--near-digest",
                    digest,
                ],
                ok,
            )
        };
        let stale = confirm("obsolete", true);
        assert_eq!(stale["state"], "near_rules");
        assert_eq!(f.ok(&["show", ver(&draft)])["record"]["state"], "candidate");
        let result = confirm(near["near_digest"].as_str().unwrap(), true);
        let record = f.ok(&["show", ver(&result)]);
        if relation == "same" {
            assert_eq!(result["id"], base["id"]);
            assert_eq!(record["record"]["kind"], "rule");
            assert_eq!(record["sources"].as_array().unwrap().len(), 3);
        } else {
            assert_eq!(record["record"]["kind"], "skill");
            assert_eq!(record["sources"].as_array().unwrap().len(), 2);
            assert_eq!(record["record"]["reference"], "manual/inspection");
            if relation == "restated" {
                assert_eq!(result["id"], base["id"]);
                assert_eq!(f.ok(&["show", ver(&base)])["record"]["state"], "superseded");
            }
            if relation == "exception" {
                assert_eq!(record["record"]["parent_id"], base["id"]);
            }
        }
    }
}

// [REQ-PROMO-1]
#[test]
fn proposal_requires_real_distinct_experiences() {
    let f = Fixture::new();
    let e = f.event();
    let utter = f.ok(&["say", "--text", "not an observation"]);
    for sources in [vec!["missing"], vec![id(&utter)], vec![id(&e), id(&e)]] {
        let mut args = vec!["skill", "propose", "--text", "body", "--scene", "scene"];
        for s in sources {
            args.extend(["--source", s]);
        }
        f.call(&args, false);
    }
    assert_eq!(f.ok(&["repeated"])["groups"], json!([]));
}

// [REQ-MEASURE-1]
#[test]
fn diff_separates_exact_exceptions_from_other_changes() {
    let f = Fixture::new();
    let path = |name| f.path(name).to_string_lossy().into_owned();
    let a = path("before");
    let b = path("after");
    let allow = path("allow");
    fs::write(
        &a,
        r#"{"cases":[{"case":"one","status":"pass","bytes":10},{"case":"two","status":"pass"}]}"#,
    )
    .unwrap();
    fs::write(&b,r#"{"cases":[{"case":"one","status":"not_met","bytes":11},{"case":"two","status":"pass"}]}"#).unwrap();
    fs::write(
        &allow,
        r#"[{"case":"one","field":"bytes","before":10,"after":11}]"#,
    )
    .unwrap();
    let r = f.ok(&["eval", "diff", &a, &b, "--allow", &allow]);
    assert_eq!(r["state"], "differences");
    assert_eq!(r["allowed"].as_array().unwrap().len(), 1);
    assert_eq!(
        r["unexplained"],
        json!([{"case":"one","field":"status","before":"pass","after":"not_met"}])
    );
    assert_eq!(f.ok(&["eval", "diff", &a, &a])["state"], "clean");
    fs::write(&b, r#"{"cases":[{"case":"one"},{"case":"one"}]}"#).unwrap();
    f.call(&["eval", "diff", &a, &b], false);
}

// [REQ-MEASURE-1]
#[test]
fn eval_run_and_paired_build_diff_use_independent_stores() {
    let f = Fixture::new();
    let scenes = f.path("scenes");
    fs::create_dir(&scenes).unwrap();
    fs::write(
        scenes.join("manifest.json"),
        r#"{"scenes":[{"scene":"S2","file":"case.json","cases":1}]}"#,
    )
    .unwrap();
    let data = json!({"scene":"S2","records":[{"kind":"utterance","text":"inspection","id":"source"},
   {"kind":"rule","id":"r","scene":"inventory","action":"check","source":"source","text":"count containers"}],
   "cases":[{"id":"test-1","query":{"op":"recall","action":"check"},"expect":{"must":["r"],"must_not":[],"fields":{},"meta":{}}}]});
    fs::write(scenes.join("case.json"), data.to_string()).unwrap();
    let scenes = scenes.to_str().unwrap();
    let binary = env!("CARGO_BIN_EXE_omoide");
    let run = f.ok(&[
        "eval", "run", "--scenes", scenes, "--scene", "S2", "--binary", binary,
    ]);
    assert_eq!(run["cases"][0]["status"], "pass");
    assert_eq!(run["cases"][0]["returned"], json!(["r"]));
    assert!(run["cases"][0]["bytes"].as_u64().unwrap() > 0);
    let compared = f.ok(&[
        "eval", "diff", binary, binary, "--builds", "--scenes", scenes, "--scene", "S2",
    ]);
    assert_eq!(
        compared,
        json!({"state":"clean","allowed":[],"unexplained":[]})
    );
    let refused = f.call(
        &[
            "eval", "run", "--scenes", scenes, "--scene", "S2", "--oracle",
        ],
        false,
    );
    assert_eq!(
        refused["message"],
        "invalid input: oracle requires --scene S1"
    );
}

// [REQ-RECALL-1]
#[test]
fn current_skills_and_rules_obey_kind_filters_and_action_priority() {
    let f = Fixture::new();
    let rule = f.rule("general instruction", false);
    let e = f.event();
    let draft = f.propose("alternate instruction", &[&e]);
    let near = f.ok(&["skill", "confirm", ver(&draft)]);
    let skill = f.ok(&[
        "skill",
        "confirm",
        ver(&draft),
        "--against",
        ver(&rule),
        "--near-digest",
        near["near_digest"].as_str().unwrap(),
        "--relation",
        "unrelated",
    ]);
    f.ok(&["used", "--version", ver(&skill)]);
    let all = f.ok(&["recall", "--action", "load"]);
    assert_eq!(all["results"][0]["kind"], "rule");
    assert_eq!(all["results"][1]["kind"], "skill");
    for (kind, record) in [("rule", rule), ("skill", skill)] {
        let r = f.ok(&["recall", "--action", "load", "--kind", kind]);
        assert_eq!(r["candidate_count"], 1);
        assert_eq!(r["matched_count"], 1);
        assert_eq!(r["results"][0]["id"], record["id"]);
    }
}

// [REQ-PROMO-1]
#[test]
fn confirmation_and_demotion_are_bound_to_the_exact_candidate_and_policy() {
    let f = Fixture::new();
    let e = f.event();
    let draft = f.propose("inspect", &[&e]);
    let confirmed = f.ok(&["skill", "confirm", ver(&draft)]);
    f.call(&["skill", "confirm", ver(&draft)], false);
    let args = [
        "outcome",
        "--version",
        ver(&confirmed),
        "--result",
        "failure",
        "--demote-after",
        "1",
        "--notice-id",
        "failure",
    ];
    let receipt = f.ok(&args);
    assert_eq!(f.ok(&args), receipt);
    let mut changed = args;
    changed[6] = "0";
    f.call(&changed, false);
    assert_eq!(
        f.ok(&["show", ver(&confirmed)])["record"]["state"],
        "candidate"
    );
    let again = f.ok(&["skill", "confirm", ver(&confirmed)]);
    assert_ne!(again["version_id"], confirmed["version_id"]);
    assert_eq!(again["id"], confirmed["id"]);
}

// [REQ-HOT-1]
#[test]
fn hot_rejects_reserved_markers_and_incomplete_headers_without_touching_the_file() {
    let f = Fixture::new();
    let file = f.path("notes");
    let p = file.to_str().unwrap();
    for value in [
        "<!-- omoide:end -->",
        "<!-- omoide:begin -->",
        "<!-- omoide:begin -->\n<!-- omoide:meta {} -->\n<!-- omoide:end -->",
        "stray <!-- omoide:begin -->",
        "<!-- omoide:begin -->\n<!-- omoide:begin -->\n<!-- omoide:end -->",
        "<!-- omoide:begin -->\n<!-- omoide:meta {} -->\n<!-- omoide:meta {} -->\n<!-- omoide:end -->",
        "<!-- omoide:begin -->\n<!-- omoide:end -->",
    ] {
        fs::write(&file, value).unwrap();
        for cmd in ["init", "export", "status"] {
            f.call(&["hot", cmd, p], false);
        }
        assert_eq!(fs::read_to_string(&file).unwrap(), value);
    }
    f.call(&["hot", "init", f.path("test.db").to_str().unwrap()], false);
}

// [REQ-JUDGE-1]
#[test]
fn reply_byte_bound_is_inclusive_and_valid_scores_keep_ordering_tiers() {
    let f = Fixture::new();
    let rule = f.rule("instruction", false);
    let e = f.event();
    let fact = f.ok(&[
        "fact",
        "add",
        "--text",
        "note",
        "--source",
        id(&e),
        "--certainty",
        "verified",
        "--target",
        "depot",
    ]);
    let script = f.path("judge.py");
    let name = script.to_str().unwrap();
    for (n, state) in [(128, "used"), (129, "failed"), (130, "failed")] {
        fs::write(
            &script,
            format!("import sys; sys.stdout.write('{{\"answers\":[]}}'.ljust({n}))"),
        )
        .unwrap();
        let r = f.ok(&[
            "recall",
            "--action",
            "load",
            "--judge",
            "python3",
            "--judge-arg",
            name,
            "--output-bytes",
            "128",
        ]);
        assert_eq!(r["judge_state"], state);
    }
    fs::write(&script,"import sys,json; q=json.load(sys.stdin); print(json.dumps({'answers':[{'version_id':c['version_id'],'score':1 if c['text']=='note' else .8} for c in q['candidates']]}))").unwrap();
    let r = f.ok(&[
        "recall",
        "--action",
        "load",
        "--target",
        "depot",
        "--judge",
        "python3",
        "--judge-arg",
        name,
    ]);
    assert_eq!(r["results"][0]["id"], rule["id"]);
    assert_eq!(r["results"][1]["id"], fact["id"]);
}

// [REQ-PROMO-1]
#[test]
fn malformed_actions_and_changed_demotion_policy_cannot_be_replayed() {
    let f = Fixture::new();
    let e = f.event();
    for actions in [
        vec!["--action", " "],
        vec!["--action", "x", "--action", "x"],
    ] {
        let mut args = vec![
            "skill",
            "propose",
            "--source",
            id(&e),
            "--text",
            "inspect",
            "--scene",
            "stock",
        ];
        args.extend(actions);
        f.call(&args, false);
    }
    let draft = f.propose("one", &[&e]);
    let skill = f.ok(&["skill", "confirm", ver(&draft)]);
    let cmd = [
        "outcome",
        "--version",
        ver(&skill),
        "--result",
        "failure",
        "--demote-after",
        "1",
        "--notice-id",
        "policy",
    ];
    f.ok(&cmd);
    let mut changed = cmd;
    changed[6] = "2";
    f.call(&changed, false);
}

// [REQ-HOT-1]
#[test]
fn pin_survives_restatement_but_does_not_spread_to_unrelated_neighbors() {
    let f = Fixture::new();
    let base = f.rule("keep", true);
    let u = f.ok(&["say", "--text", "modified"]);
    let args = [
        "rule",
        "add",
        "--source",
        id(&u),
        "--text",
        "new",
        "--scene",
        "loading",
        "--action",
        "load",
    ];
    let confirm = |relation, version| {
        let response = f.ok(&args);
        let mut finish = args.to_vec();
        finish.extend([
            "--against",
            version,
            "--relation",
            relation,
            "--near-digest",
            response["near_digest"].as_str().unwrap(),
        ]);
        f.ok(&finish)
    };
    let unrelated = confirm("unrelated", ver(&base));
    assert_eq!(f.ok(&["show", ver(&unrelated)])["record"]["pinned"], 0);
    let revision = confirm("restated", ver(&base));
    assert_eq!(f.ok(&["show", ver(&revision)])["record"]["pinned"], 1);
}

// [REQ-JUDGE-1]
#[test]
fn timed_out_judge_cannot_leave_a_writer_behind() {
    let f = Fixture::new();
    f.rule("paper", false);
    let script = f.path("helper.sh");
    let leaked = f.path("late-write");
    let started = f.path("started");
    fs::write(
        &script,
        "echo started > \"$2\"; ( sleep 1; echo late > \"$1\" ) & wait\n",
    )
    .unwrap();
    let r = f.ok(&[
        "recall",
        "--scene",
        "paper",
        "--judge",
        "/bin/sh",
        "--judge-arg",
        script.to_str().unwrap(),
        "--judge-arg",
        leaked.to_str().unwrap(),
        "--judge-arg",
        started.to_str().unwrap(),
        "--timeout-ms",
        "400",
    ]);
    assert_eq!(r["judge_error"], "judge timeout");
    assert!(started.exists());
    std::thread::sleep(std::time::Duration::from_millis(800));
    assert!(!leaked.exists());
}

// [REQ-PROMO-1]
#[test]
fn demotion_occurs_at_threshold_and_preserves_legacy_notice_fingerprints() {
    use omoide_store::{Sqlite, Statement, Store, Value as SqlValue};
    use sha2::{Digest, Sha256};
    let f = Fixture::new();
    let e = f.event();
    let draft = f.propose("check", &[&e]);
    let current = f.ok(&["skill", "confirm", ver(&draft)]);
    let notice = |id, threshold| {
        f.ok(&[
            "outcome",
            "--version",
            ver(&current),
            "--notice-id",
            id,
            "--result",
            "failure",
            "--demote-after",
            threshold,
        ])
    };
    notice("first", "0");
    let mut db = Sqlite::open(&f.path("test.db"), false).unwrap();
    let sets = db
        .read_batch(&[Statement::new(
            "SELECT digest FROM receipts WHERE op='first'",
            vec![],
        )])
        .unwrap();
    let old = json!(["outcome", ver(&current), null, null, "failure", null]).to_string();
    assert_eq!(
        sets[0][0][0],
        SqlValue::Text(format!("{:x}", Sha256::digest(old)))
    );
    notice("second", "3");
    assert_eq!(f.ok(&["show", ver(&current)])["record"]["state"], "current");
    notice("third", "3");
    assert_eq!(
        f.ok(&["show", ver(&current)])["record"]["state"],
        "candidate"
    );
}

// [REQ-RULE-1]
#[test]
fn pending_procedure_does_not_replace_current_show_by_record_id() {
    let f = Fixture::new();
    let base = f.rule("current", false);
    let e = f.event();
    let draft = f.propose("proposal", &[&e]);
    let near = f.ok(&["skill", "confirm", ver(&draft)]);
    f.ok(&[
        "skill",
        "confirm",
        ver(&draft),
        "--against",
        ver(&base),
        "--relation",
        "unsure",
        "--near-digest",
        near["near_digest"].as_str().unwrap(),
    ]);
    assert_eq!(
        f.ok(&["show", id(&base)])["record"]["version_id"],
        base["version_id"]
    );
}

// [REQ-JUDGE-1]
#[test]
fn judged_equal_scores_preserve_both_priority_arms() {
    let f = Fixture::new();
    let source = f.ok(&["say", "--text", "allowed"]);
    let decision = f.ok(&[
        "decide",
        "--source",
        id(&source),
        "--text",
        "fold",
        "--speech",
        "instruction",
        "--state",
        "decided",
        "--at",
        "1",
    ]);
    let e = f.event();
    let verified = f.ok(&[
        "fact",
        "add",
        "--source",
        id(&e),
        "--text",
        "fold",
        "--certainty",
        "verified",
        "--at",
        "2",
    ]);
    let low = f.ok(&[
        "fact",
        "add",
        "--source",
        id(&source),
        "--text",
        "fold",
        "--certainty",
        "reported",
        "--at",
        "3",
    ]);
    let script = f.path("equal.py");
    fs::write(&script,"import sys,json; q=json.load(sys.stdin); print(json.dumps({'answers':[{'version_id':c['version_id'],'score':0.75} for c in q['candidates']]}))").unwrap();
    let r = f.ok(&[
        "recall",
        "--scene",
        "fold",
        "--judge",
        "python3",
        "--judge-arg",
        script.to_str().unwrap(),
    ]);
    assert_eq!(r["judge_state"], "used");
    assert_eq!(
        r["results"]
            .as_array()
            .unwrap()
            .iter()
            .map(id)
            .collect::<Vec<_>>(),
        [id(&verified), id(&decision), id(&low)]
    );
}

// [REQ-MEASURE-1]
#[test]
fn diff_distinguishes_missing_fields_and_context_changes() {
    let f = Fixture::new();
    let a = f.path("before");
    let b = f.path("after");
    fs::write(
        &a,
        r#"{"cases":[{"case":"one","extra":null}],"variant":"baseline"}"#,
    )
    .unwrap();
    fs::write(&b, r#"{"cases":[{"case":"one"}],"variant":"other"}"#).unwrap();
    let result = f.ok(&["eval", "diff", a.to_str().unwrap(), b.to_str().unwrap()]);
    assert_eq!(result["unexplained"][0]["before_present"], true);
    assert_eq!(result["unexplained"][0]["after_present"], false);
    assert_eq!(result["unexplained"][1]["case"], "$study");
}

// [REQ-MEASURE-1]
#[test]
fn export_log_describes_published_bytes_and_current_rules_only_are_measured() {
    use omoide_store::{Sqlite, Statement, Store, Value as SqlValue};
    let f = Fixture::new();
    let rule = f.rule("base", true);
    let e = f.event();
    let draft = f.propose("manual", &[&e]);
    let near = f.ok(&["skill", "confirm", ver(&draft)]);
    f.ok(&[
        "skill",
        "confirm",
        ver(&draft),
        "--against",
        ver(&rule),
        "--relation",
        "unrelated",
        "--near-digest",
        near["near_digest"].as_str().unwrap(),
    ]);
    let file = f.path("section");
    let path = file.to_str().unwrap();
    f.ok(&["hot", "init", path]);
    let export = f.ok(&["hot", "export", path, "--call-id", "written"]);
    let stats = f.ok(&["stats"]);
    assert_eq!(stats["rules"].as_array().unwrap().len(), 1);
    assert_eq!(stats["rules"][0]["id"], rule["id"]);
    let mut db = Sqlite::open(&f.path("test.db"), false).unwrap();
    let data = db
        .read_batch(&[Statement::new(
            "SELECT kind,bytes,records FROM calls WHERE id='written'",
            vec![],
        )])
        .unwrap();
    assert_eq!(data[0][0][0], SqlValue::Text("hot_export".into()));
    assert_eq!(
        data[0][0][1],
        SqlValue::Integer(fs::metadata(&file).unwrap().len() as i64)
    );
    assert_eq!(export["section_bytes"], fs::metadata(file).unwrap().len());
    let SqlValue::Text(ids) = &data[0][0][2] else {
        panic!("expected exported IDs")
    };
    assert_eq!(
        serde_json::from_str::<Value>(ids)
            .unwrap()
            .as_array()
            .unwrap()
            .len(),
        2
    );
}

// [REQ-HOT-1]
#[test]
fn only_missing_text_files_can_be_initialized() {
    let f = Fixture::new();
    let path = f.path("missing");
    let name = path.to_str().unwrap();
    for cmd in ["status", "export"] {
        f.call(&["hot", cmd, name], false);
        assert!(!f.path("missing.omoide.lock").exists());
    }
    let looped = f.path("loop");
    std::os::unix::fs::symlink(&looped, &looped).unwrap();
    f.call(&["hot", "init", looped.to_str().unwrap()], false);
    assert!(!f.path("loop.omoide.lock").exists());
    fs::write(&path, [0xff, 0xfe, 0xca]).unwrap();
    f.call(&["hot", "init", name], false);
    assert_eq!(fs::read(&path).unwrap(), [0xff, 0xfe, 0xca]);
}

// [REQ-HOT-1]
#[test]
fn equivalent_registration_can_explicitly_pin_existing_rule() {
    let f = Fixture::new();
    let rule = f.rule("inspect", false);
    let source = f.ok(&["say", "--text", "keep this one"]);
    let mut args = vec![
        "rule",
        "add",
        "--source",
        id(&source),
        "--text",
        "inspect",
        "--scene",
        "loading",
        "--action",
        "load",
        "--pinned",
    ];
    let near = f.ok(&args);
    args.extend([
        "--relation",
        "same",
        "--against",
        ver(&rule),
        "--near-digest",
        near["near_digest"].as_str().unwrap(),
    ]);
    assert_eq!(f.ok(&args)["state"], "same");
    assert_eq!(f.ok(&["show", ver(&rule)])["record"]["pinned"], 1);
}

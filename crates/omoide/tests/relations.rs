use serde_json::Value;
use std::process::Command;

struct Db(tempfile::TempDir);
impl Db {
    fn new() -> Self {
        let db = Self(tempfile::tempdir().unwrap());
        db.ok(&["init"]);
        db
    }
    fn run(&self, args: &[&str], success: bool) -> Value {
        let out = Command::new(env!("CARGO_BIN_EXE_omoide"))
            .arg("--db")
            .arg(self.0.path().join("db"))
            .args(args)
            .output()
            .unwrap();
        let v = serde_json::from_slice(&out.stdout).unwrap();
        assert_eq!(out.status.success(), success, "{v}");
        v
    }
    fn ok(&self, args: &[&str]) -> Value {
        self.run(args, true)
    }
    fn words(&self, body: &str) -> String {
        self.ok(&["say", "--text", body])["id"]
            .as_str()
            .unwrap()
            .into()
    }
    fn rule<'a>(source: &'a str, body: &'a str) -> Vec<&'a str> {
        vec![
            "rule", "add", "--source", source, "--text", body, "--scene", "packing", "--action",
            "pack",
        ]
    }
}
fn field<'a>(v: &'a Value, key: &str) -> &'a str {
    v[key].as_str().unwrap()
}
fn finish<'a>(
    mut args: Vec<&'a str>,
    near: &'a Value,
    base: &'a Value,
    relation: &'a str,
) -> Vec<&'a str> {
    args.extend([
        "--near-digest",
        field(near, "near_digest"),
        "--against",
        field(base, "version_id"),
        "--relation",
        relation,
    ]);
    args
}

// [REQ-RULE-1]
#[test]
fn digest_and_exact_version_bind_the_choice() {
    let db = Db::new();
    let a = db.words("first");
    let b = db.words("second");
    let c = db.words("third");
    let initial = db.ok(&Db::rule(&a, "first"));
    let proposal = Db::rule(&b, "second");
    let near = db.ok(&proposal);
    let added = db.ok(&finish(Db::rule(&c, "third"), &near, &initial, "unrelated"));
    let stale = db.ok(&finish(proposal.clone(), &near, &initial, "restated"));
    assert_eq!(stale["state"], "near_rules");
    assert_eq!(stale["near_rules"].as_array().unwrap().len(), 2);
    assert_ne!(near["near_digest"], stale["near_digest"]);
    let mut wrong = finish(proposal.clone(), &stale, &initial, "restated");
    let index = wrong.iter().position(|v| *v == "--against").unwrap();
    wrong[index + 1] = "not-a-version";
    db.run(&wrong, false);
    let mut replace = finish(proposal, &stale, &initial, "restated");
    replace.extend(["--operation-id", "change-once"]);
    let new = db.ok(&replace);
    assert_eq!(new, db.ok(&replace));
    assert_eq!(new["id"], initial["id"]);
    assert_ne!(new["version_id"], initial["version_id"]);
    let shown = db.ok(&["show", field(&initial, "id"), "--history"]);
    let h = shown["history"].as_array().unwrap();
    assert_eq!(h.len(), 2);
    assert_eq!(h[0]["record"]["state"], "superseded");
    assert_eq!(h[0]["record"]["successor"], new["version_id"]);
    assert_eq!(h[0]["source"]["text"], "first");
    assert_eq!(h[1]["source"]["text"], "second");
    assert_eq!(
        db.ok(&["show", field(&added, "id")])["source"]["text"],
        "third"
    );
}

// [REQ-REC-1]
#[test]
fn same_adds_evidence_without_creating_versions() {
    let db = Db::new();
    let a = db.words("first");
    let b = db.words("other wording");
    let base = db.ok(&Db::rule(&a, "first"));
    let args = Db::rule(&b, "paraphrase");
    let near = db.ok(&args);
    let same = finish(args, &near, &base, "same");
    assert_eq!(db.ok(&same)["version_id"], base["version_id"]);
    db.ok(&same);
    let shown = db.ok(&["show", field(&base, "id"), "--history"]);
    assert_eq!(shown["history"].as_array().unwrap().len(), 1);
    assert_eq!(shown["sources"].as_array().unwrap().len(), 2);
    assert_eq!(shown["sources"][1]["text"], "other wording");
}

// [REQ-RECALL-1]
#[test]
fn scoped_exception_replaces_parent_only_for_specified_target() {
    let db = Db::new();
    let a = db.words("rule");
    let b = db.words("exception");
    let base = db.ok(&Db::rule(&a, "dry parchment"));
    let mut draft = Db::rule(&b, "use mesh");
    draft.extend(["--target", "annex"]);
    let near = db.ok(&draft);
    let child = db.ok(&finish(draft, &near, &base, "exception"));
    for (target, expected) in [("annex", &child), ("hall", &base)] {
        let r = db.ok(&["recall", "--scene", "parchment", "--target", target]);
        assert_eq!(r["results"].as_array().unwrap().len(), 1);
        assert_eq!(r["results"][0]["id"], expected["id"]);
        if target == "annex" {
            assert_eq!(r["results"][0]["parent_id"], base["id"]);
        }
    }
    assert_eq!(
        db.ok(&["recall", "--action", "pack"])["results"]
            .as_array()
            .unwrap()
            .len(),
        2
    );
}

// [REQ-RULE-1]
#[test]
fn pending_exposes_both_sources_and_requires_an_explicit_winner() {
    for keep in ["current", "pending"] {
        let db = Db::new();
        let a = db.words("old words");
        let b = db.words("new words");
        let base = db.ok(&Db::rule(&a, "old body"));
        let args = Db::rule(&b, "obsidian");
        let near = db.ok(&args);
        let pending = db.ok(&finish(args, &near, &base, "unsure"));
        let conflict = &pending["unresolved_conflicts"][0];
        assert_eq!(conflict["current"]["source"]["text"], "old words");
        assert_eq!(conflict["pending"]["source"]["text"], "new words");
        let recall = db.ok(&["recall", "--scene", "obsidian"]);
        assert_eq!(
            recall["results"][0]["unresolved_conflicts"][0]["pending"]["record"]["version_id"],
            pending["version_id"]
        );
        let command = [
            "resolve",
            field(&pending, "version_id"),
            "--keep",
            keep,
            "--operation-id",
            "choice",
        ];
        let resolved = db.ok(&command);
        assert_eq!(resolved, db.ok(&command));
        let chosen = if keep == "current" { &base } else { &pending };
        assert_eq!(resolved["version_id"], chosen["version_id"]);
        let after = db.ok(&["recall", "--action", "pack"]);
        assert_eq!(after["results"][0]["version_id"], chosen["version_id"]);
        assert!(after["results"][0].get("unresolved_conflicts").is_none());
    }
}

// [REQ-DEC-1]
#[test]
fn summary_fact_defaults_to_unknown_and_keeps_full_source() {
    let db = Db::new();
    let original = "Use paper only indoors.\nNever use it in rain.";
    let source = db.words(original);
    let args = [
        "fact",
        "add",
        "--text",
        "Use paper",
        "--source",
        &source,
        "--operation-id",
        "fact",
    ];
    let fact = db.ok(&args);
    assert_eq!(fact, db.ok(&args));
    let shown = db.ok(&["show", field(&fact, "id")]);
    assert_eq!(shown["record"]["certainty"], "unknown");
    assert_eq!(shown["sources"][0]["text"], original);
    db.run(
        &[
            "fact",
            "add",
            "--source",
            "missing",
            "--text",
            "unsupported",
        ],
        false,
    );
}

// [REQ-STORE-1]
#[test]
fn competing_restatements_do_not_apply_a_choice_to_another_version() {
    let db = Db::new();
    let source = db.words("base words");
    let base = db.ok(&Db::rule(&source, "base"));
    let mut attempts = Vec::new();
    for text in ["revision one", "revision two"] {
        let source = db.words(text);
        let draft = Db::rule(&source, text);
        let near = db.ok(&draft);
        let args = finish(draft, &near, &base, "restated");
        attempts.push(args.into_iter().map(str::to_owned).collect::<Vec<_>>());
    }
    let gate = std::sync::Barrier::new(2);
    let mut results = std::thread::scope(|scope| {
        let workers: Vec<_> = attempts
            .iter()
            .map(|args| {
                scope.spawn(|| {
                    gate.wait();
                    db.ok(&args.iter().map(String::as_str).collect::<Vec<_>>())
                })
            })
            .collect();
        workers
            .into_iter()
            .map(|w| w.join().unwrap())
            .collect::<Vec<_>>()
    });
    results.sort_by_key(|r| r["state"].as_str().unwrap().to_owned());
    assert_eq!(results[0]["state"], "near_rules");
    assert_eq!(results[1]["state"], "replaced");
    assert_eq!(
        results[0]["near_rules"][0]["record"]["version_id"],
        results[1]["version_id"]
    );
    let h = db.ok(&["show", field(&base, "id"), "--history"]);
    assert_eq!(h["history"].as_array().unwrap().len(), 2);
    assert_eq!(
        h["history"][1]["record"]["text"],
        h["history"][1]["source"]["text"]
    );
}

// [REQ-RULE-1]
#[test]
fn exception_cannot_expand_past_its_parent_during_revision() {
    let db = Db::new();
    let source = db.words("small workshop");
    let mut initial = Db::rule(&source, "parent");
    initial.extend(["--target", "shed"]);
    let base = db.ok(&initial);
    let mut scoped = Db::rule(&source, "first exception");
    scoped.extend(["--target", "shed"]);
    let near = db.ok(&scoped);
    let child = db.ok(&finish(scoped, &near, &base, "exception"));
    let mut outside = Db::rule(&source, "move exception");
    outside.extend(["--target", "quarry"]);
    let near = db.ok(&outside);
    db.run(&finish(outside, &near, &child, "restated"), false);
}

// [REQ-RECALL-1]
#[test]
fn exception_preserves_applicable_pending_evidence() {
    let db = Db::new();
    let source = db.words("parent words");
    let child_src = db.words("exception words");
    let pending_src = db.words("disputed words");
    let base = db.ok(&Db::rule(&source, "parent body"));
    let mut scoped = Db::rule(&child_src, "child body");
    scoped.extend(["--target", "shed"]);
    let near = db.ok(&scoped);
    let child = db.ok(&finish(scoped, &near, &base, "exception"));
    let dispute = Db::rule(&pending_src, "disputed body");
    let near = db.ok(&dispute);
    db.ok(&finish(dispute, &near, &base, "unsure"));
    let r = db.ok(&["recall", "--action", "pack", "--target", "shed"]);
    assert_eq!(r["results"].as_array().unwrap().len(), 1);
    assert_eq!(r["results"][0]["id"], child["id"]);
    assert_eq!(
        r["results"][0]["unresolved_conflicts"][0]["pending"]["source"]["text"],
        "disputed words"
    );
}

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

impl Db {
    // Capture the schema, metadata (including revision), receipts, FTS shadow tables
    // and all user tables. Compare logical rows, independent of WAL checkpoints.
    fn snapshot(&self) -> Vec<(String, omoide_store::Rows)> {
        use omoide_store::{Sqlite, Statement, Store, Value as SqlValue};
        let mut store = Sqlite::open(&self.0.path().join("db"), false).unwrap();
        let schema = store
            .read_batch(&[Statement::new(
                "SELECT name, sql FROM sqlite_schema WHERE type='table' ORDER BY name",
                vec![],
            )])
            .unwrap()
            .remove(0);
        let names: Vec<_> = schema
            .iter()
            .map(|row| match &row[0] {
                SqlValue::Text(name) => name.clone(),
                _ => panic!("expected table name"),
            })
            .collect();
        let queries = names
            .iter()
            .map(|name| {
                Statement::new(
                    format!("SELECT * FROM \"{}\"", name.replace('"', "\"\"")),
                    vec![],
                )
            })
            .collect::<Vec<_>>();
        let mut result = vec![("sqlite_schema".into(), schema)];
        result.extend(names.into_iter().zip(store.read_batch(&queries).unwrap()));
        result
    }
    fn pending(&self, base: &Value, body: &str) -> Value {
        let source = self.words(body);
        let args = Db::rule(&source, body);
        let near = self.ok(&args);
        let result = self.ok(&finish(args, &near, base, "unsure"));
        assert_eq!(result["state"], "pending");
        result
    }
    fn assert_refused(&self, args: &[&str], message: &str) {
        let before = self.snapshot();
        let error = self.run(args, false);
        assert_eq!(error["kind"], "invalid_input");
        assert_eq!(error["message"], format!("invalid input: {message}"));
        assert_eq!(self.snapshot(), before);
    }
    fn assert_version_state(&self, version: &Value, expected: &str) {
        let record = &self.ok(&["show", field(version, "version_id")])["record"];
        assert_eq!(record["version_id"], version["version_id"]);
        assert_eq!(record["state"], expected);
    }
}

// [REQ-RULE-1]
#[test]
fn restatement_waits_for_open_proposal_against_that_version() {
    let db = Db::new();
    let source = db.words("current words");
    let current = db.ok(&Db::rule(&source, "base"));
    let pending = db.pending(&current, "alternative");
    let revision_source = db.words("new revision");
    let args = Db::rule(&revision_source, "new revision");
    let near = db.ok(&args);
    let revision = finish(args, &near, &current, "restated");
    db.assert_refused(
        &revision,
        "resolve pending conflicts before restating this version",
    );
    db.assert_version_state(&current, "current");
    db.assert_version_state(&pending, "pending");
    db.ok(&[
        "resolve",
        field(&pending, "version_id"),
        "--keep",
        "current",
    ]);
    assert_eq!(db.ok(&revision)["state"], "replaced");
}

// [REQ-RULE-1]
#[test]
fn adoption_waits_for_other_proposals_against_that_version() {
    let db = Db::new();
    let source = db.words("current words");
    let current = db.ok(&Db::rule(&source, "base"));
    let first = db.pending(&current, "alternative one");
    let other = db.pending(&current, "alternative two");
    let adopt = ["resolve", field(&first, "version_id"), "--keep", "pending"];
    db.assert_refused(
        &adopt,
        "resolve other pending proposals against this version first",
    );
    db.assert_version_state(&current, "current");
    for v in [&first, &other] {
        db.assert_version_state(v, "pending");
    }
    db.ok(&["resolve", field(&other, "version_id"), "--keep", "current"]);
    assert_eq!(db.ok(&adopt)["version_id"], first["version_id"]);
}

// [REQ-RULE-1]
#[test]
fn parent_and_child_scope_bounds_distinguish_global_from_named_targets() {
    let db = Db::new();
    let source = db.words("packing");
    let base = db.ok(&Db::rule(&source, "base"));
    let proposal = |base: &Value, relation: &str, target: Option<&str>, success| {
        let mut args = Db::rule(&source, "changed instruction");
        if let Some(target) = target {
            args.extend(["--target", target]);
        }
        let near = db.ok(&args);
        db.run(&finish(args, &near, base, relation), success)
    };
    let child = proposal(&base, "exception", Some("hall"), true);
    // A global parent admits a child with any *named* target, but an exception must stay scoped.
    let no_target = proposal(&child, "restated", None, false);
    assert_eq!(
        no_target["message"],
        "invalid input: proposed target conflicts with a linked exception scope"
    );
    let moved = proposal(&child, "restated", Some("annex"), true);
    // A parent's new scope must still contain its current child.
    let excludes_child = proposal(&base, "restated", Some("hall"), false);
    assert_eq!(
        excludes_child["message"],
        "invalid input: proposed target conflicts with a linked exception scope"
    );
    let bounded = proposal(&base, "restated", Some("annex"), true);
    let outside = proposal(&bounded, "exception", Some("hall"), false);
    assert_eq!(
        outside["message"],
        "invalid input: exception target is outside the parent scope"
    );
    let unchanged_scope = proposal(&moved, "restated", Some("annex"), true);
    db.assert_version_state(&unchanged_scope, "current");
    assert_eq!(
        proposal(&bounded, "restated", None, true)["state"],
        "replaced"
    );
}

// [REQ-RULE-1]
#[test]
fn disjoint_targets_require_a_distinct_choice_from_an_open_conflict() {
    let db = Db::new();
    let source = db.words("packing");
    let mut original = Db::rule(&source, "base");
    original.extend(["--target", "east"]);
    let current = db.ok(&original);
    let mut draft = Db::rule(&source, "new instruction");
    draft.extend(["--target", "west"]);
    let near = db.ok(&draft);
    let conflict = finish(draft.clone(), &near, &current, "unsure");
    db.assert_refused(
        &conflict,
        "disjoint targets cannot form an applicable conflict",
    );
    assert_eq!(
        db.ok(&finish(draft, &near, &current, "restated"))["state"],
        "replaced"
    );
}

// [REQ-RULE-1]
#[test]
fn hidden_neighbor_invalidates_digest_and_cannot_be_selected() {
    let db = Db::new();
    let source = db.words("Label the packing carton.");
    let proposal = Db::rule(&source, "carton seal");
    let base = db.ok(&proposal);
    let mut versions = vec![field(&base, "version_id").to_owned()];
    for _ in 1..10 {
        let preview = db.ok(&proposal);
        let pick = &preview["near_rules"][0]["record"];
        let next = db.ok(&finish(proposal.clone(), &preview, pick, "unrelated"));
        versions.push(field(&next, "version_id").to_owned());
    }
    versions.sort();
    let shown = db.ok(&proposal);
    let neighbors = shown["near_rules"].as_array().unwrap();
    assert_eq!(neighbors.len(), 9);
    assert_eq!(shown["near_remaining"], 1);
    assert_eq!(
        neighbors
            .iter()
            .map(|v| field(&v["record"], "version_id"))
            .collect::<Vec<_>>(),
        versions[..9]
    );
    assert!(
        neighbors
            .iter()
            .all(|v| v["near_score"] == 300 && v["source"]["text"] == "Label the packing carton.")
    );
    let mut hidden = finish(proposal.clone(), &shown, &neighbors[0]["record"], "same");
    let at = hidden.iter().position(|v| *v == "--against").unwrap() + 1;
    hidden[at] = &versions[9];
    db.run(&hidden, false);
    let weak = Db::rule(&source, "carton seal spare tag");
    let weak_preview = db.ok(&weak);
    db.ok(&finish(
        weak,
        &weak_preview,
        &weak_preview["near_rules"][0]["record"],
        "unrelated",
    ));
    let stale = db.ok(&finish(
        proposal.clone(),
        &shown,
        &neighbors[0]["record"],
        "restated",
    ));
    assert_eq!(stale["state"], "near_rules");
    assert_eq!(stale["near_rules"], shown["near_rules"]);
    assert_eq!(stale["near_remaining"], 2);
    assert_ne!(stale["near_digest"], shown["near_digest"]);
    let mut no_relation = proposal.clone();
    no_relation.extend(["--near-digest", field(&stale, "near_digest")]);
    db.run(&no_relation, false);
    assert_eq!(db.ok(&proposal)["near_digest"], stale["near_digest"]);
}

// [REQ-RULE-1]
#[test]
fn lexical_fields_and_witnessed_target_reach_the_registration_score() {
    let d = Db::new();
    let src = d.words("An invented procedure");
    let args = |body: &'static str, scene: &'static str| {
        vec![
            "rule",
            "add",
            "--source",
            src.as_str(),
            "--text",
            body,
            "--scene",
            scene,
            "--target",
            "shed",
            "--action",
            "pack",
        ]
    };
    assert_eq!(
        d.ok(&args("linen wax chalk", "rain shed tile"))["state"],
        "activated"
    );
    let near = d.ok(&args("linen iron", "rain shed tent"));
    assert_eq!(near["near_rules"][0]["near_score"], 84);
    let mut alias = args("ink", "dusk");
    alias.extend(["--alias", "quartz"]);
    assert_eq!(d.ok(&alias)["state"], "activated");
    let from_alias = d.ok(&args("quartz", "night"));
    assert_eq!(from_alias["state"], "near_rules");
    assert_eq!(from_alias["near_rules"][0]["record"]["text"], "ink");
}

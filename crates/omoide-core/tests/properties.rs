use omoide_core::*;
use proptest::prelude::*;

// [REQ-RECALL-1]
#[test]
fn tokens_are_normalized_and_ignore_separators() {
    proptest!(|(a in "[A-Z0-9]{1,10}", b in "[ぁ-ん一-龯]{1,10}")| {
        let parts=tokens(&format!("{a} / {b}"));
        prop_assert_eq!(&parts,&tokens(&format!("{}　!!!{b}",a.to_lowercase())));
        prop_assert!(parts.words.contains(&a.to_lowercase()));
        prop_assert_eq!(parts.singles.len(), b.chars().collect::<std::collections::BTreeSet<_>>().len());
    });
}
// [REQ-RECALL-1]
#[test]
fn additional_candidate_preserves_pair_order() {
    proptest!(|(q in "[abc ]{1,12}", a in "[abc ]{1,12}", b in "[abc ]{1,12}", c in "[abc ]{1,12}", actions in any::<[bool;3]>())| {
        let query=tokens(&q);
        let rows=[("a",&a),("b",&b),("c",&c)];
        let candidates=||rows.iter().enumerate().map(|(i,(id,body))|Candidate{id,target_only:false,priority:false,uses:0,terms:tokens(body),created:i as i64,action:actions[i]});
        let pair=rank(&query,candidates().take(2));
        let mut all=rank(&query,candidates());
        all.retain(|r|r.id!="c");
        prop_assert_eq!(all,pair);
    });
}
// [REQ-RECALL-1]
#[test]
fn japanese_windows_and_exact_words() {
    let q = tokens("ＡＢＣ８ 猫と魚。犬");
    assert_eq!(
        q.words.into_iter().collect::<Vec<_>>(),
        ["abc8", "と魚", "犬", "猫と"]
    );
    assert_eq!(hits(&tokens("魚"), &tokens("金魚")), 1);
    assert_eq!(hits(&tokens("英語"), &tokens("英国")), 0);
    assert_eq!(hits(&tokens("abc"), &tokens("abcd")), 0);
}
// [REQ-RULE-1]
#[test]
fn registration_table() {
    use Outcome::*;
    use Relation::*;
    for (relation, expected) in [
        (None, NeedRelation),
        (Some(Same), AddSource),
        (Some(Revision), Replace),
        (Some(Exception), ActivateException),
        (Some(Unrelated), Activate),
        (Some(Unclear), Pending),
    ] {
        assert_eq!(registration(true, relation), expected);
        assert_eq!(
            registration(false, relation),
            if relation.is_some() {
                Invalid
            } else {
                Activate
            }
        );
    }
}

// [REQ-RECALL-1]
#[test]
fn character_channel_and_field_boundaries() {
    for (word, single) in [
        ("紙", true),
        ("紙箱", false),
        ("", false),
        ("x", false),
        ("1", false),
    ] {
        assert_eq!(single_japanese(word), single);
    }
    let doc = document("紙", "箱", &["折り方".into(), "CHECK".into()]);
    assert_eq!(hits(&tokens("紙 箱 折り方 check"), &doc), 5);
    assert_eq!(hits(&tokens("紙箱"), &doc), 0);
}

// [REQ-RULE-1]
#[test]
fn neighbors_by_action_or_two_distinct_terms() {
    let candidate = tokens("red paper");
    for (query, action, expected) in [
        ("blue", false, false),
        ("red red", false, false),
        ("red paper", false, true),
        ("blue", true, true),
    ] {
        assert_eq!(near_rule(&tokens(query), &candidate, action), expected);
    }
}

// [REQ-RECALL-1]
#[test]
fn rank_orders_actions_then_coverage_and_breaks_ties() {
    let records = [
        ("d", "none", false, 9),
        ("c", "red", false, 2),
        ("b", "red", false, 2),
        ("e", "red", false, 1),
        ("f", "red paper", false, 1),
        ("a", "none", true, 0),
    ];
    let result = rank(
        &tokens("red paper"),
        records
            .into_iter()
            .map(|(id, doc, action, created)| Candidate {
                target_only: false,
                priority: false,
                uses: 0,
                id,
                terms: tokens(doc),
                action,
                created,
            }),
    );
    assert_eq!(
        result.iter().map(|r| r.id).collect::<Vec<_>>(),
        ["a", "f", "b", "c", "e"]
    );
    for pair in result.windows(2) {
        assert_eq!(
            pair[0].partial_cmp(&pair[1]),
            Some(std::cmp::Ordering::Less)
        );
    }
}

// [REQ-RECALL-1]
#[test]
fn pages_keep_offsets_byte_budget_and_complete_sentences() {
    use omoide_core::page::page;
    use serde_json::json;
    let make = |text: &str| json!({"id":"rule","version_id":"version","text":text});
    let run = |cursor, records: &[_]| page(cursor, 7, records.len(), false, records);
    let singleton = [make("")];
    let base = run(None, &singleton).unwrap().to_string().len();
    let exact = "a".repeat(3999 - base);
    let output = run(None, &[make(&exact)]).unwrap();
    assert_eq!(output.to_string().len(), 3999);
    assert_eq!(output["results"][0]["text"], exact);
    for extra in [1, 2, 9] {
        let output = run(None, &[make(&(exact.clone() + &"a".repeat(extra)))]).unwrap();
        assert_eq!(output["results"][0]["truncated"], true);
        assert_eq!(output["results"][0]["text"], "");
    }
    let large = "紙。".to_owned() + &"b".repeat(5000) + ".";
    let rows = [make("one"), make("two"), make(&large), make("four")];
    let first = run(None, &rows).unwrap();
    assert_eq!(first["remaining"], 2);
    assert_eq!(first["cursor"], "7:2");
    let second = run(Some("7:2"), &rows).unwrap();
    assert_eq!(second["results"][0]["text"], "紙。");
    assert_eq!(second["remaining"], 1);
    assert_eq!(second["cursor"], "7:3");
    let last = run(Some("7:3"), &rows).unwrap();
    assert_eq!(last["results"][0]["text"], "four");
    assert_eq!(run(Some("7:4"), &rows).unwrap()["results"], json!([]));
    assert!(run(Some("7:5"), &rows).is_err());
    assert_eq!(run(Some("6:3"), &rows).unwrap()["restarted"], true);
    let huge =
        json!({"id":"rule","version_id":"version","text":"body","condition":"c".repeat(4000)});
    assert!(run(None, &[huge]).is_err());
}

// [REQ-RECALL-1]
#[test]
fn shortened_records_use_the_actual_next_cursor_and_largest_fitting_sentence() {
    use serde_json::json;
    // Independently serialize the expected page. Cursor length changes at the end
    // of results and at ten records; those bytes also belong to the 4,000-byte cap.
    for (offset, tail) in [(0usize, 0usize), (1, 0), (9, 1), (10, 9)] {
        let total = offset + 1 + tail;
        let next = offset + 1;
        let expected = |record| {
            json!({
                "state":"found","judge_state":"not_configured","results":[record],"candidate_count":total,
                "matched_count":total,"candidates_truncated":false,"more_search_available":false,
                "remaining":tail,"cursor":(tail>0).then(||format!("7:{next}")),"restarted":false
            })
        };
        let mut rows = vec![json!({"text":"other"}); total];
        let cursor = format!("7:{offset}");
        for byte_delta in -4isize..=4 {
            let shortened = json!({"id":"r","version_id":"v","text":"","truncated":true});
            let overhead = expected(shortened.clone()).to_string().len();
            let prefix =
                "a".repeat((3999isize - overhead as isize - 3 + byte_delta) as usize) + "。";
            rows[offset] =
                json!({"id":"r","version_id":"v","text":format!("{prefix}{}!", "x".repeat(4200))});
            let mut maximal = shortened.clone();
            maximal["text"] = json!(prefix);
            let actual = omoide_core::page::page(Some(&cursor), 7, total, false, &rows).unwrap();
            let wanted = if expected(maximal.clone()).to_string().len() < 4000 {
                maximal
            } else {
                shortened.clone()
            };
            assert_eq!(actual, expected(wanted));
            // Required metadata must fit even after removing every sentence.
            let needed = 3999isize
                - expected(json!({"id":"r","version_id":"v","text":"","truncated":true,"scope":""}))
                    .to_string()
                    .len() as isize
                + byte_delta;
            let scope = "s".repeat(needed as usize);
            rows[offset] = json!({"id":"r","version_id":"v","text":"x".repeat(4200),"scope":scope});
            let lookup = omoide_core::page::page(Some(&cursor), 7, total, false, &rows);
            assert_eq!(lookup.is_ok(), byte_delta <= 0);
        }
    }
}

// [REQ-RECALL-1]
#[test]
fn oversized_inherited_conflict_identifies_original_evidence() {
    let entry = serde_json::json!({"version_id":"child","text":"body","unresolved_conflicts":[
        {"pending":{"record":{"version_id":"candidate"},"source":{"text":"紙".repeat(2000)}}}
    ]});
    let error = omoide_core::page::page(None, 1, 1, false, &[entry]).unwrap_err();
    assert!(error.contains("child") && error.contains("candidate"));
}

// [REQ-PROMO-1]
#[test]
fn target_evidence_priority_and_capped_usage_respect_applicability() {
    let query = tokens("one two");
    let cases = [
        ("a", true, false, false, 0, "none", 1),
        ("b", false, false, true, 0, "one", 1),
        ("c", false, false, false, 0, "one two", 1),
        ("d", false, false, false, 43, "one", 1),
        ("e", false, false, false, 32, "one", 2),
        ("f", false, true, true, 0, "none", 1),
        ("g", false, true, false, 32, "none", 2),
        ("h", false, true, false, 0, "none", 3),
        ("excluded", false, false, true, 100, "none", 9),
    ];
    let ranked = rank(
        &query,
        cases
            .into_iter()
            .map(
                |(id, action, target_only, priority, uses, doc, created)| Candidate {
                    id,
                    action,
                    target_only,
                    priority,
                    uses,
                    terms: tokens(doc),
                    created,
                },
            ),
    );
    assert_eq!(
        ranked.iter().map(|r| r.id).collect::<Vec<_>>(),
        ["a", "b", "c", "e", "d", "f", "h", "g"]
    );
    assert_eq!(ranked[4].uses, 32);
    assert_eq!(ranked[2].hits, 2);
}

// [REQ-RECALL-1]
#[test]
fn usage_revision_and_accounting_fields_are_in_the_actual_page_budget() {
    let mut meta = serde_json::Map::new();
    meta.insert("call_id".into(), serde_json::json!("call"));
    let rows = vec![serde_json::json!({"version_id":"v","text":"a".repeat(2200)}); 3];
    let run = |cursor, usage| {
        omoide_core::page::with_metadata(cursor, 8, usage, 3, false, &rows, &meta).unwrap()
    };
    assert_eq!(run(None, 1)["cursor"], "8.1:1");
    assert_eq!(run(Some("8.1:1"), 1)["remaining"], 1);
    assert_eq!(run(Some("8.1:1"), 2)["restarted"], true);
    assert_eq!(run(Some("7.1:1"), 1)["restarted"], true);
    assert_eq!(run(Some("8:1"), 0)["restarted"], false);
    meta.insert("extra".into(), serde_json::json!("x".repeat(4000)));
    assert!(omoide_core::page::with_metadata(None, 1, 1, 0, false, &[], &meta).is_err());
}

// [REQ-JUDGE-1]
#[test]
fn judge_is_all_or_nothing_except_explicit_empty() {
    use omoide_core::judge::scores;
    let ids = ["a".to_owned(), "b".to_owned(), "c".to_owned()]
        .into_iter()
        .collect();
    let valid=br#"{"answers":[{"version_id":"a","score":0},{"version_id":"b","score":0.5},{"version_id":"c","score":1}]}"#;
    let selected = scores(valid, &ids, 0.5).unwrap();
    assert!(!selected.contains_key("a"));
    assert_eq!((selected["b"], selected["c"]), (1, 2));
    assert!(scores(br#"{"answers":[]}"#, &ids, 1.).unwrap().is_empty());
    for bad in [
        r#"{"answers":[{"version_id":"a","score":0.8}]}"#,
        r#"{"answers":[{"version_id":"a","score":1},{"version_id":"a","score":1},{"version_id":"c","score":1}]}"#,
        r#"{"answers":[{"version_id":"other","score":0}]}"#,
        r#"{"answers":[{"version_id":"a","score":1.01}]}"#,
        "{}",
        "garbage",
    ] {
        assert!(scores(bad.as_bytes(), &ids, 0.5).is_err());
    }
    assert!(scores(valid, &ids, -0.1).is_err());
}

// [REQ-JUDGE-1]
#[test]
fn complete_replies_still_require_unique_ids_and_each_score_in_range() {
    use omoide_core::judge::scores;
    let ids = ["x".to_owned()].into_iter().collect();
    for reply in [
        r#"{"answers":[{"version_id":"x","score":-0.01}]}"#,
        r#"{"answers":[{"version_id":"x","score":1.01}]}"#,
        r#"{"answers":[{"version_id":"x","score":1},{"version_id":"x","score":0.5}]}"#,
        r#"{"answers":[{"version_id":"x","score":1},{"version_id":"y","score":0.5}]}"#,
    ] {
        assert!(scores(reply.as_bytes(), &ids, 0.5).is_err());
    }
}

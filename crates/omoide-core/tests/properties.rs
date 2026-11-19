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
        let candidates=||rows.iter().enumerate().map(|(i,(id,body))|Candidate{id,terms:tokens(body),created:i as i64,action:actions[i]});
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

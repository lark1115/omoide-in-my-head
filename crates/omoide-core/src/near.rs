//! Evidence for registration. The recall tokenizer and its index stay unchanged.
use crate::{Tokens, tokens};
use std::collections::BTreeSet;

type Terms = BTreeSet<String>;
const FLOOR: usize = 25;

fn content(text: &str) -> Terms {
    informative(tokens(text).words)
}

fn informative(mut words: Terms) -> Terms {
    words.retain(|w| {
        !w.chars().all(|c| ('ぁ'..='ゟ').contains(&c))
            && !matches!(
                w.as_str(),
                "a" | "an"
                    | "the"
                    | "to"
                    | "of"
                    | "for"
                    | "in"
                    | "on"
                    | "at"
                    | "as"
                    | "and"
                    | "or"
                    | "by"
                    | "is"
                    | "are"
                    | "it"
                    | "with"
                    | "from"
                    | "then"
                    | "when"
            )
    });
    words
}

/// Token-only compatibility API for callers without separate trigger and target fields.
pub fn near_rule(query: &Tokens, record: &Tokens, action_overlap: bool) -> bool {
    let a = informative(query.words.clone());
    let b = informative(record.words.clone());
    let shared = a.intersection(&b).count();
    shared > 0 && 100 * shared / a.len().min(b.len()) + 8 * usize::from(action_overlap) >= FLOOR
}

/// Subject words or ideographs that are actually present in the trigger.
/// Ignore ordinary timing characters when comparing Japanese compound nouns.
fn subject_units(text: &str) -> Terms {
    let mut units: Terms = content(text).into_iter().filter(|w| w.is_ascii()).collect();
    units.extend(
        text.chars()
            .filter(|c| matches!(*c, '㐀'..='鿿') && !"時前後際中場合".contains(*c))
            .map(|c| c.to_string()),
    );
    units
}

pub struct RuleProfile<'a> {
    text: &'a str,
    scene: &'a str,
    document: Terms,
    trigger: Terms,
    cues: Terms,
    aliases: Terms,
    body: Terms,
    identifiers: Terms,
    subject: Terms,
    target: Option<&'a str>,
    actions: Terms,
}

impl<'a> RuleProfile<'a> {
    pub fn new(
        text: &'a str,
        scene: &'a str,
        aliases: &[String],
        actions: &[String],
        target: Option<&'a str>,
    ) -> Self {
        let action_text = actions.join(" ");
        let target = target.filter(|s| !s.is_empty());
        let identifiers = content(&format!("{} {action_text}", target.unwrap_or_default()));
        let scene_terms = content(scene);
        let body = content(text);
        let mut document: Terms = body.union(&scene_terms).cloned().collect();
        let aliases: Terms = aliases.iter().flat_map(|s| content(s)).collect();
        document.extend(aliases.iter().cloned());
        let trimmed = scene.trim_end();
        let cue = ["場面", "場合", "とき", "時", "際"]
            .iter()
            .find_map(|suffix| trimmed.strip_suffix(suffix))
            .unwrap_or(trimmed);
        let cues = content(cue)
            .intersection(&document)
            .filter(|w| !identifiers.contains(*w))
            .cloned()
            .collect();
        let trigger = scene_terms.difference(&identifiers).cloned().collect();
        let mut subject = subject_units(scene);
        if let Some(t) = target {
            let named = subject_units(t);
            subject.retain(|s| named.contains(s));
        }
        let verbs = subject_units(&action_text);
        subject.retain(|s| !verbs.contains(s));
        Self {
            document,
            trigger,
            cues,
            aliases,
            body,
            identifiers,
            subject,
            text,
            scene,
            target,
            actions: actions.iter().cloned().collect(),
        }
    }
}

pub fn score(a: &RuleProfile, b: &RuleProfile) -> Option<usize> {
    // Exact descriptions (including kana-only rules) outrank term-set similarity.
    if !a.text.is_empty() && a.text == b.text && !a.scene.is_empty() && a.scene == b.scene {
        return Some(300);
    }

    let action = !a.actions.is_disjoint(&b.actions);
    let sibling = action && a.target != b.target && !a.subject.is_disjoint(&b.subject);
    let mut shared = 0;
    let mut anchor = false;
    for word in a.document.intersection(&b.document) {
        shared += 1;
        anchor |= !a.identifiers.contains(word) && !b.identifiers.contains(word);
    }
    if !anchor {
        return sibling.then_some(FLOOR);
    }
    let coverage = 100 * shared / a.document.len().min(b.document.len());
    let trigger = a.trigger.intersection(&b.trigger).take(2).count();
    let identical = (!a.trigger.is_empty() && a.trigger == b.trigger)
        || (!a.body.is_empty() && a.body == b.body);
    let same_target =
        a.target.is_some() && a.target == b.target && !a.subject.is_disjoint(&b.subject);
    let score = coverage
        + 8 * trigger
        + 8 * usize::from(action)
        + 8 * usize::from(action && same_target)
        + 100 * usize::from(identical);
    if sibling {
        Some(score.max(FLOOR))
    } else {
        (score >= FLOOR).then_some(score)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    // [REQ-RULE-1]
    #[test]
    fn registration_terms_keep_content_not_grammar() {
        assert_eq!(
            content("から する の the with The ＰＡＰＥＲ 紙箱"),
            Terms::from(["paper".into(), "紙箱".into()])
        );
        assert_eq!(
            subject_units("時 前 後 際 中 場 合 幕 から Paper"),
            Terms::from(["幕".into(), "paper".into()])
        );
        assert!(!near_rule(&tokens("から報告"), &tokens("から遊具"), true));
        assert!(!near_rule(
            &tokens("one two three four five"),
            &tokens("one six seven eight nine"),
            false
        ));
        assert!(near_rule(
            &tokens("one two three four"),
            &tokens("one six seven eight"),
            false
        ));
        assert!(near_rule(
            &tokens("one two three four five"),
            &tokens("one six seven eight nine"),
            true
        ));
    }

    // [REQ-RULE-1]
    #[test]
    fn fraction_uses_smaller_set_and_accepts_the_boundary() {
        let p = |s| RuleProfile::new(s, "", &[], &[], None);
        assert_eq!(
            score(&p("oak wax tin mud"), &p("oak ink red ice sun")),
            Some(25)
        );
        assert_eq!(
            score(&p("oak wax tin mud hay"), &p("oak ink red ice sun")),
            None
        );
        assert_eq!(score(&p("wax oak"), &p("wax red ink iron tin")), Some(50));
        let a = RuleProfile::new("wax", "hold", &["purple".into()], &[], None);
        assert_eq!(score(&a, &p("purple")), Some(100));
    }

    // [REQ-RULE-1]
    #[test]
    fn scene_and_structured_priors_require_corroboration() {
        let actions = vec!["pack".to_owned()];
        let a = RuleProfile::new(
            "linen wax chalk",
            "rain shed tile",
            &[],
            &actions,
            Some("shed"),
        );
        let b = RuleProfile::new("linen iron", "rain shed tent", &[], &actions, Some("shed"));
        // Three of five terms, one independent trigger, action and witnessed subject.
        assert_eq!(score(&a, &b), Some(84));
        let b = RuleProfile::new("linen iron", "rain shed tent", &[], &[], Some("shed"));
        assert_eq!(score(&a, &b), Some(68));
        let a = RuleProfile::new("wax", "close", &[], &actions, Some("tray"));
        let b = RuleProfile::new("metal", "open", &[], &actions, Some("tray"));
        assert_eq!(score(&a, &b), None);
        let a = RuleProfile::new("paper", "tray", &[], &actions, Some("tray"));
        let b = RuleProfile::new("glass", "tray", &[], &actions, Some("tray"));
        assert_eq!(score(&a, &b), None);
        let b = RuleProfile::new("glass", "other", &[], &actions, Some("tray"));
        assert_eq!(score(&a, &b), None);
    }

    // [REQ-RULE-1]
    #[test]
    fn sibling_noun_must_be_witnessed_in_both_triggers() {
        let action = vec!["store".to_owned()];
        let a = RuleProfile::new("干す", "雨幕", &[], &action, Some("雨幕"));
        let b = RuleProfile::new("畳む", "月幕", &[], &action, Some("月幕"));
        assert_eq!(score(&a, &b), Some(FLOOR));
        for other in [
            RuleProfile::new("畳む", "月幕", &[], &[], Some("月幕")),
            RuleProfile::new("畳む", "夜間", &[], &action, Some("月幕")),
            RuleProfile::new("畳む", "月幕", &[], &action, Some("本棚")),
        ] {
            assert_eq!(score(&a, &other), None);
        }
        assert_eq!(
            score(&a, &RuleProfile::new("畳む", "幕間", &[], &action, None)),
            Some(FLOOR)
        );
        let verb = vec!["幕".to_string()];
        assert_eq!(
            score(
                &RuleProfile::new("板", "雨幕", &[], &verb, Some("雨幕")),
                &RuleProfile::new("土", "月幕", &[], &verb, Some("月幕"))
            ),
            None
        );
    }

    // [REQ-RULE-1]
    #[test]
    fn strongest_match_includes_exact_kana_only_description() {
        let p = RuleProfile::new("ひもをしめる", "もどすとき", &[], &[], None);
        assert_eq!(score(&p, &p), Some(300));
        assert_eq!(
            score(
                &p,
                &RuleProfile::new("つちをならす", "うえるとき", &[], &[], None)
            ),
            None
        );
        let a = RuleProfile::new("red paper", "iron wax clay", &[], &[], None);
        let b = RuleProfile::new("blue paper", "iron wax clay", &[], &[], None);
        assert_eq!(score(&a, &b), Some(196));
        let c = RuleProfile::new("red paper", "rain", &[], &[], None);
        assert_eq!(score(&a, &c), Some(166));
    }

    proptest! {
        // [REQ-RULE-1]
        #[test]
        fn symmetric_under_registration_order(a in "[abc秋春夏 ]{1,25}",b in "[abc秋春夏 ]{1,25}",same_action in any::<bool>()){
            let x=RuleProfile::new(&a,"朝の棚",&[],&["a".into()],Some("北棚"));
            let y=RuleProfile::new(&b,"夜の棚",&[],&[if same_action{"a"}else{"b"}.into()],Some("南棚"));
            prop_assert_eq!(score(&x,&y),score(&y,&x));
        }
    }
}

#[cfg(test)]
mod boundaries {
    use super::*;

    // [REQ-RULE-1]
    #[test]
    fn empty_and_multi_hit_token_only_inputs() {
        assert!(!near_rule(&tokens("から"), &tokens("する"), true));
        assert!(near_rule(
            &tokens("oak wax tin ink red"),
            &tokens("oak wax mud ice sun"),
            false
        ));
    }

    // [REQ-RULE-1]
    #[test]
    fn an_identifier_for_either_side_cannot_supply_the_only_anchor() {
        let a = RuleProfile::new("blue shed", "dawn", &[], &[], Some("shed"));
        let b = RuleProfile::new("shed", "night", &[], &[], None);
        assert_eq!(score(&a, &b), None);
        assert_eq!(score(&b, &a), None);
    }

    // [REQ-RULE-1]
    #[test]
    fn extra_scope_weight_requires_a_named_subject_in_both_triggers() {
        let acts = vec!["pack".to_owned()];
        let make = |target| RuleProfile::new("linen wax", "rain north", &[], &acts, target);
        let b = RuleProfile::new("linen tin", "rain south", &[], &acts, Some("shed"));
        // Shared linen/rain: 50 coverage + 8 for the trigger + 8 for action.
        assert_eq!(score(&make(Some("shed")), &b), Some(66));
        assert_eq!(
            score(
                &make(None),
                &RuleProfile::new("linen tin", "rain south", &[], &acts, None)
            ),
            Some(66)
        );
    }
}

/// Document frequencies for the proposed description in a single store snapshot.
#[derive(Default)]
pub struct Corpus {
    pub current: usize,
    pub words: std::collections::BTreeMap<String, usize>,
    pub singles: std::collections::BTreeMap<String, usize>,
}
impl Corpus {
    fn weight(&self, term: &str, single: bool) -> usize {
        let map = if single { &self.singles } else { &self.words };
        let df = map.get(term).copied().unwrap_or(0);
        // The one document being compared is not evidence that its vocabulary is common.
        100usize.saturating_sub(200 * df.saturating_sub(1) / self.current.saturating_sub(1).max(1))
    }
    pub fn score(&self, a: &RuleProfile, b: &RuleProfile) -> Option<usize> {
        let base = score(a, b)?;
        // Repeated literal boilerplate in different scenes is not an independent anchor.
        if a.text == b.text
            && (a.scene == b.scene
                || a.body.iter().any(|w| {
                    !a.identifiers.contains(w)
                        && !b.identifiers.contains(w)
                        && self.weight(w, false) >= 75
                }))
        {
            return Some(base);
        }
        if !a.cues.is_empty() && a.cues == b.cues {
            return Some(base);
        }
        let shared = a
            .document
            .intersection(&b.document)
            .filter(|w| !a.identifiers.contains(*w) && !b.identifiers.contains(*w));
        let mut mass = 0;
        let mut penalty = 0;
        let mut alias = false;
        for word in shared {
            let w = self.weight(word, false);
            mass += w;
            penalty += 100 - w;
            alias |= w >= 75 && (a.aliases.contains(word) || b.aliases.contains(word));
        }
        let mut scene_mass = 0;
        let mut distinctive = false;
        for word in a.cues.intersection(&b.cues) {
            let w = self.weight(word, false);
            scene_mass += w;
            distinctive |= w >= 75;
        }
        let sibling = !a.actions.is_disjoint(&b.actions)
            && a.target != b.target
            && a.subject
                .intersection(&b.subject)
                .any(|w| self.weight(w, !w.is_ascii()) >= 75);
        let smaller = a.document.len().min(b.document.len()).max(1);
        let scene_match = distinctive && scene_mass >= 20 * a.cues.len().min(b.cues.len());
        if sibling || ((scene_match || alias) && mass >= 4 * smaller) {
            Some(base.saturating_sub(penalty / smaller).max(1))
        } else {
            None
        }
    }
}

#[cfg(test)]
mod corpus_tests {
    use super::*;
    use proptest::prelude::*;

    fn corpus() -> Corpus {
        Corpus {
            current: 9,
            words: [("common".into(), 9), ("seed".into(), 2)].into(),
            singles: [("幕".into(), 2)].into(),
        }
    }
    // [REQ-RULE-1]
    #[test]
    fn weights_use_distinct_current_documents_and_independent_columns() {
        let c = corpus();
        assert_eq!(c.weight("missing", false), 100);
        assert_eq!(c.weight("seed", false), 75);
        assert_eq!(c.weight("common", false), 0);
        assert_eq!(c.weight("幕", true), 75);
        assert_eq!(c.weight("幕", false), 100);
        assert_eq!(Corpus::default().weight("missing", false), 100);
    }
    // [REQ-RULE-1]
    #[test]
    fn shared_boilerplate_needs_a_distinctive_scene_or_rare_explicit_alias() {
        let act = vec!["pack".into()];
        let a = RuleProfile::new("red common", "seed left", &[], &act, None);
        let b = RuleProfile::new("blue common", "seed right", &[], &act, None);
        let mut c = corpus();
        assert_eq!(c.score(&a, &b), Some(35));
        let identified = RuleProfile::new("red common", "seed left", &[], &act, Some("common"));
        assert_eq!(c.score(&identified, &b), Some(60));
        assert_eq!(c.score(&b, &identified), Some(60));
        c.words.insert("seed".into(), 3);
        assert_eq!(c.score(&a, &b), None);
        let alias = RuleProfile::new("blue common", "another place", &["rare".into()], &act, None);
        let other = RuleProfile::new("red common rare", "entirely different", &[], &act, None);
        assert!(c.score(&alias, &other).is_some());
        assert_eq!(c.score(&other, &alias), c.score(&alias, &other));
        c.words.insert("rare".into(), 3);
        assert_eq!(c.score(&other, &alias), None);
        let c1 = RuleProfile::new("red common", "床を磨く場面", &[], &act, None);
        let c2 = RuleProfile::new("blue common", "箱を運ぶ場面", &[], &act, None);
        assert_eq!(c.score(&c1, &c2), None);
        let same = RuleProfile::new("different common", "床を磨く場合", &[], &act, None);
        assert!(c.score(&c1, &same).is_some());
        let identical_body = RuleProfile::new("red common", "entirely different", &[], &act, None);
        assert!(c.score(&c1, &identical_body).is_some());
    }
    // [REQ-RULE-1]
    #[test]
    fn a_frequent_identical_instruction_needs_an_independent_cue() {
        let acts = vec!["pack".into()];
        let a = RuleProfile::new("common", "open gate", &[], &acts, None);
        let b = RuleProfile::new("common", "weigh paper", &[], &acts, None);
        let c = corpus();
        assert_eq!(c.score(&a, &b), None);
        assert_eq!(c.score(&a, &a), Some(300));
        let named = RuleProfile::new("common", "weigh paper", &[], &acts, Some("common"));
        assert_eq!(Corpus::default().score(&a, &named), None);
        assert_eq!(Corpus::default().score(&named, &a), None);
    }
    // [REQ-RULE-1]
    #[test]
    fn sibling_uses_the_frequency_of_its_written_subject() {
        let act = vec!["store".into()];
        let a = RuleProfile::new("乾かす", "雨幕", &[], &act, Some("雨幕"));
        let b = RuleProfile::new("巻く", "月幕", &[], &act, Some("月幕"));
        let mut c = corpus();
        assert_eq!(c.score(&a, &b), Some(FLOOR));
        c.singles.insert("幕".into(), 3);
        assert_eq!(c.score(&a, &b), None);
    }
    // [REQ-RULE-1]
    #[test]
    fn scene_and_document_mass_boundaries_are_inclusive() {
        let act = vec!["pack".into()];
        for (tail, passes) in [(4, true), (5, false)] {
            let left = format!(
                "seed {}",
                (0..tail)
                    .map(|i| format!("left{i}"))
                    .collect::<Vec<_>>()
                    .join(" ")
            );
            let right = left.replace("left", "right");
            let a = RuleProfile::new("first", &left, &[], &act, None);
            let b = RuleProfile::new("second", &right, &[], &act, None);
            assert_eq!(Corpus::default().score(&a, &b).is_some(), passes);
        }
        for (n, passes) in [(22, true), (23, false)] {
            let left = (0..n)
                .map(|i| format!("left{i}"))
                .collect::<Vec<_>>()
                .join(" ");
            let right = left.replace("left", "right");
            let a = RuleProfile::new(&left, "seed shed north", &[], &act, Some("shed"));
            let b = RuleProfile::new(&right, "seed shed south", &[], &act, Some("shed"));
            assert_eq!(Corpus::default().score(&a, &b).is_some(), passes);
        }
    }
    proptest! {
        // [REQ-RULE-1]
        #[test]
        fn more_documents_containing_a_term_never_raise_its_weight(n in 1usize..1024, a in 0usize..1024,b in 0usize..1024){
            let mut c=Corpus{current:n,..Default::default()};
            c.words.insert("sample".into(),a.min(b).min(n));
            let before=c.weight("sample",false);
            c.words.insert("sample".into(),a.max(b).min(n));
            prop_assert!(c.weight("sample",false)<=before);
        }
    }
}

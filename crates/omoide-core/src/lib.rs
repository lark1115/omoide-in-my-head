pub mod page;
use std::{
    cmp::{Ordering, Reverse},
    collections::BTreeSet,
};
use unicode_normalization::UnicodeNormalization;

#[derive(Default, Debug, PartialEq, Eq)]
pub struct Tokens {
    pub words: BTreeSet<String>,
    pub singles: BTreeSet<String>,
}
fn japanese(c: char) -> bool {
    matches!(c as u32,0x3041..=0x3096|0x309d..=0x309f|0x30a1..=0x30fa|0x30fc..=0x30ff|0x31f0..=0x31ff|0x3400..=0x4dbf|0x4e00..=0x9fff|0x20000..=0x323af|0x3005|0x3007)
}
pub fn tokens(text: &str) -> Tokens {
    let mut out = Tokens::default();
    let mut latin = String::new();
    let mut prev = None;
    let mut length = 0;
    for c in text.nfkc().flat_map(char::to_lowercase).chain([' ']) {
        if japanese(c) {
            if !latin.is_empty() {
                out.words.insert(std::mem::take(&mut latin));
            }
            out.singles.insert(c.to_string());
            if let Some(p) = prev {
                out.words.insert(format!("{p}{c}"));
            }
            prev = Some(c);
            length += 1;
        } else {
            if length == 1 {
                out.words.insert(prev.unwrap().to_string());
            }
            prev = None;
            length = 0;
            if c.is_alphanumeric() {
                latin.push(c);
            } else if !latin.is_empty() {
                out.words.insert(std::mem::take(&mut latin));
            }
        }
    }
    out
}
pub fn single_japanese(word: &str) -> bool {
    let mut chars = word.chars();
    chars.next().is_some_and(japanese) && chars.next().is_none()
}
pub fn hits(query: &Tokens, doc: &Tokens) -> usize {
    query
        .words
        .iter()
        .filter(|w| {
            if single_japanese(w) {
                doc.singles.contains(*w)
            } else {
                doc.words.contains(*w)
            }
        })
        .count()
}
/// All candidates share the query denominator, so integer hits order the coverage fraction exactly.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Rank<'a> {
    pub action: bool,
    pub target_only: bool,
    pub priority: bool,
    pub uses: u32,
    pub hits: usize,
    pub created: i64,
    pub id: &'a str,
}
impl Ord for Rank<'_> {
    fn cmp(&self, other: &Self) -> Ordering {
        (
            Reverse(self.action),
            self.target_only,
            Reverse(self.priority),
            Reverse(self.hits),
            Reverse(self.uses),
            Reverse(self.created),
            self.id,
        )
            .cmp(&(
                Reverse(other.action),
                other.target_only,
                Reverse(other.priority),
                Reverse(other.hits),
                Reverse(other.uses),
                Reverse(other.created),
                other.id,
            ))
    }
}
impl PartialOrd for Rank<'_> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Relation {
    Same,
    Revision,
    Exception,
    Unrelated,
    Unclear,
}
#[derive(Debug, PartialEq, Eq)]
pub enum Outcome {
    Activate,
    Invalid,
    NeedRelation,
    AddSource,
    Replace,
    ActivateException,
    Pending,
}
pub fn registration(near: bool, relation: Option<Relation>) -> Outcome {
    use Outcome::*;
    match (near, relation) {
        (false, None) | (true, Some(Relation::Unrelated)) => Activate,
        (false, Some(_)) => Invalid,
        (true, None) => NeedRelation,
        (true, Some(Relation::Same)) => AddSource,
        (true, Some(Relation::Revision)) => Replace,
        (true, Some(Relation::Exception)) => ActivateException,
        (true, Some(Relation::Unclear)) => Pending,
    }
}
pub fn document(text: &str, scene: &str, aliases: &[String]) -> Tokens {
    // Separate fields to avoid creating a bigram across a field boundary.
    tokens(
        &std::iter::once(text)
            .chain(std::iter::once(scene))
            .chain(aliases.iter().map(String::as_str))
            .collect::<Vec<_>>()
            .join(" "),
    )
}

pub fn near_rule(query: &Tokens, record: &Tokens, action_overlap: bool) -> bool {
    action_overlap || hits(query, record) >= 2
}

pub struct Candidate<'a> {
    pub id: &'a str,
    pub action: bool,
    pub target_only: bool,
    pub priority: bool,
    pub uses: u32,
    pub created: i64,
    pub terms: Tokens,
}
pub fn rank<'a>(
    query: &Tokens,
    candidates: impl IntoIterator<Item = Candidate<'a>>,
) -> Vec<Rank<'a>> {
    let mut ranked = Vec::new();
    for candidate in candidates {
        let coverage = hits(query, &candidate.terms);
        if candidate.action || coverage > 0 || candidate.target_only {
            ranked.push(Rank {
                id: candidate.id,
                created: candidate.created,
                action: candidate.action,
                hits: coverage,
                target_only: candidate.target_only,
                priority: candidate.priority,
                uses: if candidate.target_only {
                    0
                } else {
                    candidate.uses.min(32)
                },
            });
        }
    }
    ranked.sort();
    ranked
}
pub mod judge;

pub mod selection;

pub mod relevance;
pub mod tiers;

//! Lexical evidence used for first-page ordering. No corpus-size-dependent weights.
use crate::Tokens;
use rust_stemmers::{Algorithm, Stemmer};
use std::collections::BTreeSet;

pub struct LexicalQuery {
    stemmer: Stemmer,
    words: BTreeSet<String>,
}
impl LexicalQuery {
    pub fn new(query: &Tokens) -> Self {
        let stemmer = Stemmer::create(Algorithm::English);
        let words = query.words.iter().map(|t| normalize(&stemmer, t)).collect();
        Self { stemmer, words }
    }
    fn field(&self, field: &Tokens) -> usize {
        let mut strength = 0;
        for term in &field.words {
            let stem = normalize(&self.stemmer, term);
            let weight = if term.is_ascii() {
                term.len().clamp(1, 8)
            } else {
                4
            };
            if self.words.contains(&stem) {
                strength += weight * 2;
            } else if term.is_ascii()
                && term.len() >= 4
                && self.words.iter().any(|q| q.starts_with(&stem))
            {
                strength += weight;
            }
        }
        strength.saturating_mul(strength) / field.words.len().max(1)
    }
    pub fn score(&self, body: &Tokens, subject: &Tokens) -> usize {
        self.field(body) + self.field(subject)
    }
}
fn normalize(stemmer: &Stemmer, term: &str) -> String {
    if term.is_ascii() {
        stemmer.stem(term).into_owned()
    } else {
        term.to_owned()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tokens;
    use proptest::prelude::*;
    // [REQ-RECALL-1]
    #[test]
    fn exact_compound_stem_and_distinct_subject_contributions() {
        let q = LexicalQuery::new(&tokens("packing archway 青"));
        assert_eq!(q.score(&tokens("packed"), &tokens("")), 144);
        assert_eq!(q.score(&tokens(""), &tokens("arch")), 16);
        assert_eq!(q.score(&tokens("青"), &tokens("青")), 128);
        assert_eq!(q.score(&tokens("noise"), &tokens("")), 0);
        assert_eq!(q.score(&tokens("archway noise"), &tokens("")), 98);
    }
    proptest! {
        // [REQ-RECALL-1]
        #[test]
        fn unrelated_terms_do_not_raise_field_relevance(n in 0usize..90){
            let q=LexicalQuery::new(&tokens("packing"));
            let empty=tokens("");
            let mut terms=tokens("packed");
            let before=q.score(&terms,&empty);
            for i in 0..n {terms.words.insert(format!("other{i}"));}
            prop_assert!(q.score(&terms,&empty)<=before);
        }
    }
}

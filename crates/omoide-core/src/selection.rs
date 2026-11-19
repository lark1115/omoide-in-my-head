//! Eligibility of free-text rules. Ranking keys remain independent of this cut.
use crate::{Tokens, hits};

/// Count the extra evidence supplied by trigger terms beyond the target and action.
pub fn trigger_hits(query: &Tokens, scene: &Tokens, identifiers: &Tokens) -> usize {
    let distinguishing = Tokens {
        words: scene
            .words
            .difference(&identifiers.words)
            .cloned()
            .collect(),
        singles: scene
            .singles
            .difference(&identifiers.singles)
            .cloned()
            .collect(),
    };
    hits(query, &distinguishing)
}

pub fn covers_query(hits: usize, query_terms: usize, trigger: usize, best: u128) -> bool {
    let hits = hits as u128;
    hits > 0 && hits * 100 >= query_terms as u128 * 12 && (hits + trigger as u128) * 5 / 3 >= best
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tokens;
    use proptest::prelude::*;

    // [REQ-RECALL-1]
    #[test]
    fn trigger_excludes_shared_words_and_single_characters() {
        assert_eq!(
            trigger_hits(
                &tokens("wood paint 棚 青"),
                &tokens("wood paint 棚 青"),
                &tokens("paint 青")
            ),
            2
        );
        assert_eq!(
            trigger_hits(&tokens("wood"), &tokens("metal"), &tokens("")),
            0
        );
    }

    // [REQ-RECALL-1]
    #[test]
    fn both_boundaries_are_inclusive() {
        assert!(!covers_query(0, 0, 0, 0));
        assert!(!covers_query(11, 100, 0, 11));
        assert!(covers_query(12, 100, 0, 20));
        assert!(!covers_query(12, 100, 0, 21));
        assert!(covers_query(12, 100, 1, 21));
        assert!(covers_query(
            usize::MAX,
            usize::MAX,
            usize::MAX,
            2 * usize::MAX as u128
        ));
    }
    proptest! {
        // [REQ-RECALL-1]
        #[test]
        fn selection_requires_both_minima(n in 1usize..8192,h in 0usize..8192, t in 0usize..8192, b in 0usize..16384) {
            let min_hits=(12*n).div_ceil(100);
            let min_combined=(3*b).div_ceil(5);
            prop_assert_eq!(covers_query(h,n,t,b as u128),h>=min_hits && h+t>=min_combined);
        }
    }
}

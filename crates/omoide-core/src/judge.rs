//! Validate the entire reply before accepting any score. The explicit empty list is a valid no-match.
use serde::Deserialize;
use std::collections::{BTreeMap, BTreeSet};
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Reply {
    answers: Vec<Answer>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Answer {
    version_id: String,
    score: f64,
}
pub fn scores(
    bytes: &[u8],
    ids: &BTreeSet<String>,
    threshold: f64,
) -> Result<BTreeMap<String, usize>, String> {
    if !(0.0..=1.0).contains(&threshold) {
        return Err("invalid judge threshold".into());
    }
    let reply: Reply = serde_json::from_slice(bytes).map_err(|_| "invalid judge JSON")?;
    if reply.answers.is_empty() {
        return Ok(BTreeMap::new());
    }
    let mut seen = BTreeSet::new();
    let mut accepted = Vec::new();
    for a in reply.answers {
        if !ids.contains(&a.version_id)
            || !seen.insert(a.version_id.clone())
            || !(0.0..=1.0).contains(&a.score)
        {
            return Err("invalid judge id or score".into());
        }
        if a.score >= threshold {
            accepted.push(a);
        }
    }
    if &seen != ids {
        return Err("incomplete judge reply".into());
    }
    accepted.sort_by(|a, b| a.score.total_cmp(&b.score));
    let mut previous = None;
    let mut ordinal = 0;
    Ok(accepted
        .into_iter()
        .map(|a| {
            if previous != Some(a.score) {
                ordinal += 1;
                previous = Some(a.score);
            }
            (a.version_id, ordinal)
        })
        .collect())
}

pub struct Tiers {
    pub confirmed: BTreeMap<String, usize>,
    pub candidates: BTreeMap<String, usize>,
}

pub fn tiers(bytes: &[u8], ids: &BTreeSet<String>, threshold: f64) -> Result<Tiers, String> {
    let confirmed = scores(bytes, ids, threshold)?;
    let mut candidates = scores(bytes, ids, 0.30)?;
    candidates.retain(|id, _| !confirmed.contains_key(id));
    Ok(Tiers {
        confirmed,
        candidates,
    })
}

#[cfg(test)]
mod tier_tests {
    use super::*;
    // [REQ-JUDGE-1]
    #[test]
    fn both_closed_lower_bounds_and_disjoint_membership() {
        let ids = ["a", "b", "c", "d", "e"]
            .map(String::from)
            .into_iter()
            .collect();
        let data=br#"{"answers":[{"version_id":"a","score":0.299},{"version_id":"b","score":0.30},{"version_id":"c","score":0.599},{"version_id":"d","score":0.6},{"version_id":"e","score":1.0}]}"#;
        let result = tiers(data, &ids, 0.60).unwrap();
        assert_eq!(
            result.confirmed.keys().cloned().collect::<Vec<_>>(),
            ["d", "e"]
        );
        assert_eq!(
            result.candidates.keys().cloned().collect::<Vec<_>>(),
            ["b", "c"]
        );
        assert!(result.candidates["b"] < result.candidates["c"]);
        assert!(tiers(data, &ids, 0.20).unwrap().candidates.is_empty());
        assert!(tiers(b"{}", &ids, 0.60).is_err());
    }
}

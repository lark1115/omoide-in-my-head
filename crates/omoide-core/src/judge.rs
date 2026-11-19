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

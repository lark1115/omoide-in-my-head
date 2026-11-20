//! Count the structurally eligible universe in the same snapshot as the hits.
use crate::{args::Query, db::*};
use omoide_store::Statement;
use serde_json::{Value as Json, json};
use std::collections::BTreeMap;

pub const EMPTY: &str = "The device did not find a confirmed match. This is not evidence that no record exists. Search the other sources you have.";
pub const KINDS: [&str; 5] = ["rule", "skill", "decision", "fact", "experience"];

pub fn census(query: &Query, procedures: bool) -> Statement {
    q("SELECT kind,count(*) FROM (
        SELECT r.kind FROM rules r WHERE ?1 AND r.state='current' AND (?2 IS NULL OR r.target IS NULL OR r.target=?2)
        AND (r.exclusion IS NULL OR r.exclusion IS NOT ?2) AND (?3 IS NULL OR r.kind=?3)
        AND (?4 IS NOT NULL OR EXISTS(SELECT 1 FROM action_links a WHERE a.version=r.version AND a.action=?5) OR EXISTS(SELECT 1 FROM rules p JOIN action_links a ON a.version=p.version WHERE p.pending_against=r.version AND p.state='pending' AND a.action=?5 AND (?2 IS NULL OR p.target IS NULL OR p.target=?2) AND (p.exclusion IS NULL OR p.exclusion IS NOT ?2)))
        UNION ALL SELECT m.kind FROM memories m WHERE coalesce(json_extract(m.details,'$.decision'),'')!='revised'
        AND (?2 IS NULL OR m.target IS NULL OR m.target=?2) AND (?3 IS NULL OR m.kind=?3)
        AND (?4 IS NOT NULL OR ?2 IS NOT NULL)
        AND (?6 IS NULL OR json_extract(m.details,'$.period')=?6)
        AND (?7='[]' OR EXISTS(SELECT 1 FROM json_each(m.details,'$.scope') a JOIN json_each(?7) b ON a.value=b.value))
        AND (?8 IS NULL OR json_extract(m.details,'$.claim')=?8)) GROUP BY kind UNION ALL SELECT '_current_rules', count(*) FROM rules WHERE state='current'",
        vec![i64::from(procedures).into(),opt(&query.target),opt(&query.kind),opt(&query.scene),opt(&query.action),opt(&query.period),s(&json!(query.scope).to_string()),opt(&query.claim)])
}

pub fn counts(
    rows: &omoide_store::Rows,
    pool: &BTreeMap<String, (bool, Json)>,
    judged: &std::collections::BTreeSet<String>,
) -> Result<(Json, bool, usize, usize)> {
    let mut eligible = BTreeMap::new();
    for row in rows {
        eligible.insert(text(&row[0])?, int(&row[1])? as usize);
    }
    let mut out = serde_json::Map::new();
    let mut skipped = false;
    for kind in KINDS {
        let eligible = eligible.get(kind).copied().unwrap_or_default();
        let mut scored = 0;
        let mut done = 0;
        for (_, r) in pool.values().filter(|(_, r)| r["kind"] == kind) {
            scored += 1;
            done += usize::from(
                r["version_id"]
                    .as_str()
                    .is_some_and(|id| judged.contains(id)),
            );
        }
        skipped |= eligible > scored;
        out.insert(
            kind.into(),
            json!({"eligible":eligible,"scored":scored,"judged":done}),
        );
    }
    let active = eligible.remove("_current_rules").unwrap_or_default();
    Ok((Json::Object(out), skipped, active, eligible.values().sum()))
}

#[cfg(test)]
mod tests {
    use super::*;
    // [REQ-RECALL-1]
    #[test]
    fn compare_tallies_by_version_and_kind() {
        let rows = vec![
            vec![s("_current_rules"), 4.into()],
            vec![s("rule"), 3.into()],
            vec![s("experience"), 1.into()],
        ];
        let pool = BTreeMap::from([
            ("a".into(), (true, json!({"kind":"rule","version_id":"av"}))),
            (
                "b".into(),
                (false, json!({"kind":"rule","version_id":"bv"})),
            ),
            (
                "e".into(),
                (false, json!({"kind":"experience","version_id":"ev"})),
            ),
        ]);
        let done = std::collections::BTreeSet::from(["av".into(), "ev".into()]);
        let (by_kind, skipped, active, total) = counts(&rows, &pool, &done).unwrap();
        assert!(skipped);
        assert_eq!((active, total), (4, 4));
        assert_eq!(by_kind["rule"], json!({"eligible":3,"scored":2,"judged":1}));
        assert_eq!(
            by_kind["experience"],
            json!({"eligible":1,"scored":1,"judged":1})
        );
        for k in ["skill", "decision", "fact"] {
            assert_eq!(by_kind[k], json!({"eligible":0,"scored":0,"judged":0}));
        }
    }
}

use crate::{args::Query, db::*};
use omoide_core::{Candidate, Tokens, document, near_rule, rank, tokens};
use omoide_store::{Error, Store};
use serde_json::{Value as Json, json};
use std::collections::BTreeMap;

fn stored_tokens(r: &Json) -> Tokens {
    let aliases = r["aliases"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(Json::as_str)
        .map(str::to_owned)
        .collect::<Vec<_>>();
    document(
        r["text"].as_str().unwrap_or_default(),
        r["scene"].as_str().unwrap_or_default(),
        &aliases,
    )
}
pub fn near(r: &Json, t: &Tokens, actions: &[String]) -> bool {
    let doc = stored_tokens(r);
    let action_overlap = r["actions"]
        .as_array()
        .into_iter()
        .flatten()
        .any(|a| actions.iter().any(|x| a.as_str() == Some(x)));
    near_rule(t, &doc, action_overlap)
}
pub fn recall(db: &mut impl Store, query: &Query) -> Result<Json> {
    if query.scene.as_deref().unwrap_or_default().trim().is_empty()
        && query
            .action
            .as_deref()
            .unwrap_or_default()
            .trim()
            .is_empty()
    {
        return Err(Error::Input("expected a scene or action name".into()));
    }
    if query.fts_limit == 0 {
        return Err(Error::Input("fts limit must be positive".into()));
    }
    let terms = tokens(query.scene.as_deref().unwrap_or_default());
    let queries = omoide_store::lookup::candidates(
        &terms,
        query.target.as_deref(),
        query.action.as_deref(),
        query.scan_limit,
        query.fts_limit,
    );
    let sets = db.read_batch(&queries)?;
    let rev = number(&sets[0])?;
    let mut pool = BTreeMap::new();
    let capped = sets
        .get(3)
        .is_some_and(|rows| rows.len() > query.fts_limit as usize);
    for (i, rows) in sets.iter().enumerate().skip(1) {
        for row in rows.iter().take(if i == 3 {
            query.fts_limit as usize
        } else {
            usize::MAX
        }) {
            let r = parse(&row[0])?;
            let id = r["id"]
                .as_str()
                .ok_or_else(|| Error::Unknown("missing rule id".into()))?
                .to_owned();
            pool.entry(id).or_insert((i == 1, r));
        }
    }
    let count = pool.len();
    let candidates = pool.iter().map(|(id, (action, record))| Candidate {
        id,
        action: *action,
        created: record["created"].as_i64().unwrap_or_default(),
        terms: stored_tokens(record),
    });
    let mut results = Vec::new();
    for entry in rank(&terms, candidates) {
        let mut item = pool[entry.id].1.clone();
        for key in ["purpose", "exclusion"] {
            if item[key].is_null() {
                item[key] = json!("unknown")
            }
        }
        item.as_object_mut().unwrap().remove("aliases");
        results.push(item);
    }
    omoide_core::page::page(query.cursor.as_deref(), rev, count, capped, &results)
        .map_err(Error::Input)
}

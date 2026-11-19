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
    let mut queries = omoide_store::lookup::candidates(
        &terms,
        query.target.as_deref(),
        query.action.as_deref(),
        query.scan_limit,
        query.fts_limit,
    );
    let normal_len = queries.len();
    queries.extend(omoide_store::lookup::linked_rules(
        query.target.as_deref(),
        query.action.as_deref(),
        &terms,
    ));
    let sets = db.read_batch(&queries)?;
    let (sets, links) = sets.split_at(normal_len);
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
    let mut conflicts: BTreeMap<String, Vec<Json>> = BTreeMap::new();
    let mut alternate_text: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for row in &links[1] {
        let current = parse(&row[0])?;
        let pending = parse(&row[2])?;
        let id = current["id"].as_str().unwrap().to_owned();
        let action_hit = query.action.as_deref().is_some_and(|name| {
            pending["actions"]
                .as_array()
                .unwrap()
                .iter()
                .any(|a| a == name)
        });
        let entry = pool.entry(id.clone()).or_insert((false, current.clone()));
        entry.0 |= action_hit;
        let texts = alternate_text.entry(id.clone()).or_default();
        texts.extend([
            pending["text"].as_str().unwrap().into(),
            pending["scene"].as_str().unwrap().into(),
        ]);
        texts.extend(
            pending["aliases"]
                .as_array()
                .unwrap()
                .iter()
                .filter_map(Json::as_str)
                .map(str::to_owned),
        );
        conflicts.entry(id).or_default().push(
            json!({"current":{"record":current,"source":parse(&row[1])?},
            "pending":{"record":pending,"source":parse(&row[3])?}}),
        );
    }
    let exceptions = links[0]
        .iter()
        .map(|row| parse(&row[0]))
        .collect::<Result<Vec<_>>>()?;
    let mut children: BTreeMap<&str, Vec<&Json>> = BTreeMap::new();
    for exception in &exceptions {
        children
            .entry(exception["parent_id"].as_str().unwrap())
            .or_default()
            .push(exception);
    }
    // A scoped exception may suppress the ordinary row, never a conflict attached
    // to that row. Retain the evidence independently of which member ranked first.
    if query.target.is_some() {
        let inherited = conflicts.clone();
        for (base, pairs) in &inherited {
            let mut stack: Vec<_> = children
                .get(base.as_str())
                .into_iter()
                .flatten()
                .copied()
                .collect();
            while let Some(child) = stack.pop() {
                let child_id = child["id"].as_str().unwrap();
                conflicts
                    .entry(child_id.to_owned())
                    .or_default()
                    .extend(pairs.iter().cloned());
                stack.extend(children.get(child_id).into_iter().flatten().copied());
            }
        }
    }
    let count = pool.len();
    let candidates = pool.iter().map(|(id, (action, record))| {
        let mut terms = stored_tokens(record);
        if let Some(texts) = alternate_text.get(id) {
            for text in texts {
                let other = tokens(text);
                terms.words.extend(other.words);
                terms.singles.extend(other.singles);
            }
        }
        Candidate {
            id,
            action: *action,
            created: record["created"].as_i64().unwrap_or_default(),
            terms,
        }
    });
    let mut results = Vec::new();
    let mut visited = std::collections::BTreeSet::new();
    for entry in rank(&terms, candidates) {
        let mut stack = vec![&pool[entry.id].1];
        while let Some(record) = stack.pop() {
            let id = record["id"].as_str().unwrap();
            if !visited.insert(id) {
                continue;
            }
            if let Some(scoped) = children.get(id) {
                stack.extend(scoped.iter().rev().copied());
                if query.target.is_some() {
                    continue;
                }
            }
            let mut item = record.clone();
            if let Some(pairs) = conflicts.get(id) {
                item["unresolved_conflicts"] = json!(pairs);
            }
            for key in ["purpose", "exclusion"] {
                if item[key].is_null() {
                    item[key] = json!("unknown")
                }
            }
            item.as_object_mut().unwrap().remove("aliases");
            results.push(item);
        }
    }
    omoide_core::page::page(query.cursor.as_deref(), rev, count, capped, &results)
        .map_err(Error::Input)
}

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
pub fn recall(db: &mut impl Store, query: &Query, call_id: &str) -> Result<Json> {
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
    let include_rules = query
        .kind
        .as_deref()
        .is_none_or(|k| matches!(k, "rule" | "skill"))
        && query.scope.is_empty()
        && query.period.is_none()
        && query.claim.is_none();
    let mut queries = if include_rules {
        omoide_store::lookup::candidate_queries(
            &terms,
            query.target.as_deref(),
            query.action.as_deref(),
            query.scan_limit,
            query.fts_limit,
            query.kind.as_deref(),
            query.judging.judge.is_some(),
        )
    } else {
        vec![omoide_store::revision()]
    };
    let normal_len = queries.len();
    if include_rules {
        queries.extend(omoide_store::lookup::linked_rules(
            query.target.as_deref(),
            query.action.as_deref(),
            &terms,
        ));
    }
    let evidence_index = queries.len();
    queries.push(omoide_store::evidence::candidates(
        omoide_store::evidence::Filter {
            terms: &terms,
            target: query.target.as_deref(),
            kind: query.kind.as_deref(),
            period: query.period.as_deref(),
            scope_json: &json!(query.scope).to_string(),
            claim: query.claim.as_deref(),
            limit: query.fts_limit,
        },
    ));
    queries.push(q(
        "SELECT usage_revision FROM meta WHERE singleton=1",
        vec![],
    ));
    let sets = db.read_batch(&queries)?;
    let evidence = &sets[evidence_index];
    let usage_revision = number(sets.last().unwrap())?;
    let (sets, links) = sets[..evidence_index].split_at(normal_len);
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
    for row in links.get(1).into_iter().flatten() {
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
    let exceptions = links
        .first()
        .into_iter()
        .flatten()
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
    let capped = capped || evidence.len() > query.fts_limit as usize;
    for row in evidence.iter().take(query.fts_limit as usize) {
        let mut record = parse(&row[0])?;
        record["match_basis"] = json!(if int(&row[2])? == 1 {
            "lexical"
        } else {
            "target_only"
        });
        if record["kind"] == "decision" {
            record["source_text"] = parse(&row[1])?["text"].clone();
        }
        pool.insert(record["id"].as_str().unwrap().to_owned(), (false, record));
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
            target_only: record["match_basis"] == "target_only",
            priority: record["decision"] == "decided" || record["certainty"] == "verified",
            uses: record["used_count"]
                .as_u64()
                .unwrap_or(0)
                .min(u32::MAX.into()) as u32,
            created: record["created"].as_i64().unwrap_or_default(),
            terms,
        }
    });
    let mut ranked = rank(&terms, candidates);
    let mut judge_state = "not_configured";
    let mut judge_error = None;
    if query.judging.judge.is_some() {
        let input = json!({"scene":query.scene,"action":query.action,"target":query.target});
        match crate::judge::ask(&query.judging, &input, &pool) {
            Ok(scores) => {
                judge_state = "used";
                ranked = pool
                    .iter()
                    .filter_map(|(id, (action, r))| {
                        let hits = *scores.get(r["version_id"].as_str().unwrap())?;
                        Some(omoide_core::Rank {
                            id,
                            action: *action,
                            target_only: false,
                            priority: r["decision"] == "decided" || r["certainty"] == "verified",
                            hits,
                            uses: r["used_count"].as_u64().unwrap_or(0).min(32) as u32,
                            created: r["created"].as_i64().unwrap_or(0),
                        })
                    })
                    .collect();
                ranked.sort();
            }
            Err(message) => {
                judge_state = "failed";
                judge_error = Some(message);
            }
        }
    }
    // Direct action lookup admits skills too; only an action-bound rule has the first ranking key.
    for key in &mut ranked {
        if pool[key.id].1["kind"] == "skill" {
            key.action = false;
        }
    }
    ranked.sort();
    let mut results = Vec::new();
    let mut visited = std::collections::BTreeSet::new();
    for entry in ranked {
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
            if query
                .kind
                .as_deref()
                .is_some_and(|kind| record["kind"] != kind)
            {
                continue;
            }
            let mut item = record.clone();
            if let Some(pairs) = conflicts.get(id) {
                item["unresolved_conflicts"] = json!(pairs);
            }
            for key in ["purpose", "exclusion"] {
                if item["kind"] == "rule" && item[key].is_null() {
                    item[key] = json!("unknown")
                }
            }
            item.as_object_mut().unwrap().remove("aliases");
            results.push(item);
        }
    }
    let mut meta = serde_json::Map::new();
    meta.insert("judge_state".into(), json!(judge_state));
    if let Some(err) = judge_error {
        meta.insert("judge_error".into(), json!(err));
    }
    meta.insert("call_id".into(), json!(call_id));
    meta.insert("accounting".into(), json!("recorded"));
    // Keep scoped evidence visible when a target or period is unknown, without
    // turning that evidence into an unconditional verification of the query.
    let proves = |r: &&Json| {
        r["certainty"] == "verified"
            && r["evidence"].is_string()
            && r["target"]
                .as_str()
                .is_none_or(|target| Some(target) == query.target.as_deref())
            && r["period"]
                .as_str()
                .is_none_or(|period| Some(period) == query.period.as_deref())
    };
    let mut fully_covered = true;
    if !query.scope.is_empty() {
        let covered: std::collections::BTreeSet<_> = results
            .iter()
            .filter(proves)
            .filter_map(|r| r["scope"].as_array())
            .flatten()
            .filter_map(Json::as_str)
            .collect();
        let unproven: Vec<_> = query
            .scope
            .iter()
            .filter(|s| !covered.contains(s.as_str()))
            .collect();
        fully_covered = unproven.is_empty();
        meta.insert("unproven_scope".into(), json!(unproven));
    }
    if query.claim.is_some() {
        meta.insert(
            "verified".into(),
            json!(fully_covered && results.iter().any(|r| proves(&r))),
        );
    }
    omoide_core::page::with_metadata(
        query.cursor.as_deref(),
        rev,
        usage_revision,
        count,
        capped,
        &results,
        &meta,
    )
    .map_err(Error::Input)
}

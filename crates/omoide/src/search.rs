use crate::{args::Query, db::*};
use omoide_core::{Candidate, Tokens, document, rank, tokens};
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
            query.judging.judge.is_some() && query.scene.is_some(),
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
    let census_index = queries.len();
    queries.push(crate::coverage::census(query, include_rules));
    queries.push(q(
        "SELECT usage_revision FROM meta WHERE singleton=1",
        vec![],
    ));
    let sets = db.read_batch(&queries)?;
    let census = &sets[census_index];
    let evidence = &sets[evidence_index];
    let usage_revision = number(sets.last().unwrap())?;
    let (sets, links) = sets[..evidence_index].split_at(normal_len);
    let rev = number(&sets[0])?;
    let mut pool = BTreeMap::new();
    let rule_cap = sets
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
    let evidence_cap = evidence.len() > query.fts_limit as usize;
    let capped = rule_cap || evidence_cap;
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
    pool.retain(|_, (_, r)| query.kind.as_deref().is_none_or(|kind| r["kind"] == kind));
    let count = pool.len();
    let lexical_query = omoide_core::relevance::LexicalQuery::new(&terms);
    let mut relevance = BTreeMap::new();
    let candidates = pool.iter().map(|(id, (action, record))| {
        let mut terms = stored_tokens(record);
        if let Some(texts) = alternate_text.get(id) {
            for text in texts {
                let other = tokens(text);
                terms.words.extend(other.words);
                terms.singles.extend(other.singles);
            }
        }
        let subject = tokens(record["target"].as_str().unwrap_or_default());
        let mut quality = lexical_query.score(&terms, &subject);
        if query.period.is_none()
            && record["period"]
                .as_str()
                .is_some_and(|period| !query.scene.as_deref().unwrap_or_default().contains(period))
        {
            quality /= 4;
        }
        relevance.insert(id.as_str(), quality);
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
    let ranked = rank(&terms, candidates);
    let rule_triggers: BTreeMap<_, _> = ranked
        .iter()
        .filter(|r| pool[r.id].1["kind"] == "rule")
        .map(|r| {
            let record = &pool[r.id].1;
            let identifiers = tokens(&format!(
                "{} {}",
                record["target"].as_str().unwrap_or_default(),
                record["actions"]
            ));
            let scene = tokens(record["scene"].as_str().unwrap_or_default());
            (
                r.id,
                omoide_core::selection::trigger_hits(&terms, &scene, &identifiers),
            )
        })
        .collect();
    let best = ranked
        .iter()
        .filter_map(|r| rule_triggers.get(r.id).map(|t| r.hits as u128 + *t as u128))
        .max()
        .unwrap_or(0);
    let (mut ranked, mut possible): (Vec<_>, Vec<_>) = ranked.into_iter().partition(|r| {
        r.action
            || rule_triggers.get(r.id).is_none_or(|trigger| {
                omoide_core::selection::covers_query(r.hits, terms.words.len(), *trigger, best)
            })
    });
    let mut judge_state = "not_configured";
    let mut judge_error = None;
    let mut done = std::collections::BTreeSet::new();
    let mut requested = 0;
    let mut oversized = 0;

    if query.judging.judge.is_some() {
        let input = json!({"scene":query.scene,"action":query.action,"target":query.target});
        let mut selected: Vec<_> = pool
            .iter()
            .filter(|(_, (direct, r))| {
                query.action.is_none()
                    || !matches!(r["kind"].as_str(), Some("rule" | "skill"))
                    || *direct
            })
            .collect();
        selected
            .sort_by_key(|(id, (direct, _))| std::cmp::Reverse((*direct, relevance[id.as_str()])));
        let selected = selected.into_iter().map(|(_, (_, r))| r).collect();
        match crate::judging::ask(&query.judging, &input, selected) {
            Ok(judgment) => {
                done = judgment.done;
                requested = judgment.requested;
                oversized = judgment.oversized;
                judge_error = judgment.error;
                judge_state = if judge_error.is_some() {
                    "failed"
                } else {
                    "used"
                };
                let scores = judgment.scores;
                let ordered = |scores: &BTreeMap<String, usize>| {
                    pool.iter()
                        .filter_map(|(id, (action, r))| {
                            let hits = *scores.get(r["version_id"].as_str().unwrap())?;
                            Some(omoide_core::Rank {
                                id,
                                action: *action,
                                target_only: false,
                                priority: r["decision"] == "decided"
                                    || r["certainty"] == "verified",
                                hits,
                                uses: r["used_count"].as_u64().unwrap_or(0).min(32) as u32,
                                created: r["created"].as_i64().unwrap_or(0),
                            })
                        })
                        .collect()
                };
                if judge_error.is_some() {
                    ranked.retain(|r| !done.contains(pool[r.id].1["version_id"].as_str().unwrap()));
                    possible
                        .retain(|r| !done.contains(pool[r.id].1["version_id"].as_str().unwrap()));
                } else {
                    ranked.clear();
                    possible.clear();
                }
                let confirmed: Vec<_> = ordered(&scores.confirmed);
                let candidates: Vec<_> = ordered(&scores.candidates);
                ranked.extend(confirmed);
                possible.extend(candidates);
            }
            Err(message) => {
                judge_state = "failed";
                judge_error = Some(message);
            }
        }
    }
    // Direct action lookup admits skills too; only an action-bound rule has the first ranking key.
    for key in ranked.iter_mut().chain(possible.iter_mut()) {
        if pool[key.id].1["kind"] == "skill" {
            key.action = false;
        }
    }
    ranked.sort();
    possible.sort();
    if judge_state != "used" && !terms.words.is_empty() {
        let key =
            |r: &omoide_core::Rank| std::cmp::Reverse((r.action, !r.target_only, relevance[r.id]));
        ranked.sort_by_key(key);
        possible.sort_by_key(key);
    }
    let mut candidates = Vec::new();
    let mut results = Vec::new();
    let mut visited = std::collections::BTreeSet::new();
    for (entry, confirmed) in ranked
        .into_iter()
        .map(|r| (r, true))
        .chain(possible.into_iter().map(|r| (r, false)))
    {
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
            if !confirmed {
                candidates.push(record.clone());
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
    let mut evidence_by_id: BTreeMap<String, Json> = results
        .iter()
        .filter(|r| r["kind"] == "experience")
        .map(|r| (r["id"].as_str().unwrap().to_owned(), r.clone()))
        .collect();
    let mut delivered = std::collections::BTreeSet::new();
    let mut linked = Vec::with_capacity(results.len());
    for record in results {
        if !delivered.insert(record["id"].as_str().unwrap().to_owned()) {
            continue;
        }
        let evidence = record["evidence"]
            .as_str()
            .and_then(|id| evidence_by_id.remove(id));
        linked.push(record);
        if let Some(source) = evidence
            && delivered.insert(source["id"].as_str().unwrap().to_owned())
        {
            linked.push(source);
        }
    }
    let results = linked;
    let mut meta = serde_json::Map::new();
    meta.insert("judge_state".into(), json!(judge_state));
    let (counts, skipped, active, total) = crate::coverage::counts(census, &pool, &done)?;
    let incomplete = capped || skipped || judge_error.is_some();
    let rule_path = if !include_rules {
        "not_requested"
    } else if terms.words.is_empty() && query.judging.judge.is_none() && query.action.is_none() {
        "no_search_terms"
    } else if query.scene.is_none() {
        "action_index"
    } else if active <= query.scan_limit as usize {
        "full_scan"
    } else {
        "fts"
    };
    let evidence_path = if matches!(query.kind.as_deref(), Some("rule" | "skill"))
        || (query.scene.is_none() && query.target.is_none())
    {
        "not_requested"
    } else {
        "fts"
    };

    meta.insert(
        "search".into(),
        json!({"by_kind":counts,"incomplete":incomplete,"candidate_cap":capped,
        "fallback":judge_error.is_some(),"timeout":judge_error.as_deref()==Some("judge timeout"),
        "judge_requested":requested,"not_judged":total.saturating_sub(done.len()),"judge_remaining":requested.saturating_sub(done.len()),
        "rule_path":rule_path,"action_index":include_rules && query.action.is_some(),"evidence_path":evidence_path,
        "rule_cap":rule_cap,"evidence_cap":evidence_cap,"reply_cap":false,"judge_oversized":oversized,"judge_output_cap":judge_error.as_deref()==Some("judge output limit"),
        "rule_scan_limit":query.scan_limit,"fts_limit":query.fts_limit}),
    );
    if results.is_empty() {
        meta.insert("notice".into(), json!(crate::coverage::EMPTY));
    }
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
    meta.insert("candidates".into(), json!([]));
    meta.insert(
        "candidate_guidance".into(),
        json!(omoide_core::tiers::GUIDANCE),
    );
    // Reserve a count as wide as any representable candidate count before selecting strict rows.
    meta.insert("candidates_remaining".into(), json!(usize::MAX));
    if results.is_empty() && incomplete {
        meta.insert("state".into(), json!("incomplete_search"));
    } else if results.is_empty() && !candidates.is_empty() {
        meta.insert("state".into(), json!("candidates_only"));
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
    .and_then(|mut page| {
        if page["results"].as_array().unwrap().is_empty() && page.get("notice").is_none() {
            page["notice"] = json!(crate::coverage::EMPTY);
            if page.to_string().len() >= 4000 {
                return Err("query metadata exceeds output budget".into());
            }
        }
        page["candidates_remaining"] = json!(candidates.len());
        let mut page = omoide_core::tiers::append(page, &candidates);
        page["search"]["reply_cap"] = json!(
            page["remaining"] != 0
                || page["candidates_remaining"] != 0
                || page["results"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|r| r["truncated"] == true)
        );
        Ok(page)
    })
    .map_err(Error::Input)
}

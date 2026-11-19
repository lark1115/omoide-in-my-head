use serde_json::{Value as Json, json};
// JSON is followed by exactly one newline on stdout.
fn fits(payload: &Json) -> bool {
    payload.to_string().len() < 4000
}

pub fn page(
    cursor: Option<&str>,
    rev: i64,
    count: usize,
    capped: bool,
    results: &[Json],
) -> Result<Json, String> {
    with_metadata(
        cursor,
        rev,
        0,
        count,
        capped,
        results,
        &serde_json::Map::new(),
    )
}
pub fn with_metadata(
    cursor: Option<&str>,
    rev: i64,
    usage: i64,
    count: usize,
    capped: bool,
    results: &[Json],
    metadata: &serde_json::Map<String, Json>,
) -> Result<Json, String> {
    let key = if usage == 0 {
        rev.to_string()
    } else {
        format!("{rev}.{usage}")
    };
    let mut start = 0;
    let mut restarted = false;
    if let Some(token) = cursor {
        let (r, i) = token
            .split_once(':')
            .ok_or_else(|| "invalid cursor".to_owned())?;
        let (data, uses) = r.split_once('.').unwrap_or((r, "0"));
        let revision = data.parse::<i64>().map_err(|_| "invalid cursor revision")?;
        let usage_revision = uses.parse::<i64>().map_err(|_| "invalid usage revision")?;
        let i: usize = i.parse().map_err(|_| "invalid cursor offset".to_owned())?;
        if rev == revision && usage == usage_revision {
            if i > results.len() {
                return Err("cursor out of range".to_owned());
            }
            start = i;
        } else {
            restarted = true
        }
    }
    let envelope = |items: &[Json], next: usize| {
        let mut payload = json!({
            "state": if results.is_empty() { "none" } else { "found" },
            "judge_state": "not_configured",
            "results": items,
            "candidate_count": count,
            "matched_count": results.len(),
            "candidates_truncated": capped,
            "more_search_available": capped,
            "remaining": results.len()-next,
            "cursor": (next<results.len()).then(||format!("{key}:{next}")),
            "restarted": restarted,
        });
        payload.as_object_mut().unwrap().extend(metadata.clone());
        payload
    };
    if !fits(&envelope(&[], start)) {
        return Err("query metadata exceeds output budget".into());
    }
    let mut items = Vec::new();
    for item in &results[start..] {
        items.push(item.clone());
        if !fits(&envelope(&items, start + items.len())) {
            items.pop();
            if items.is_empty() {
                let mut short = item.clone();
                short["text"] = json!("");
                short["truncated"] = json!(true);
                if !fits(&envelope(std::slice::from_ref(&short), start + 1)) {
                    let mut ids = vec![item["version_id"].clone()];
                    if let Some(pairs) = item["unresolved_conflicts"].as_array() {
                        ids.extend(
                            pairs
                                .iter()
                                .map(|pair| pair["pending"]["record"]["version_id"].clone()),
                        );
                    }
                    return Err(format!(
                        "required evidence exceeds the output budget; retrieve these versions with show: {}",
                        json!(ids)
                    ));
                }
                let body = item["text"].as_str().unwrap_or_default();
                let mut end = 0;
                for (i, ch) in body.char_indices().take(4000) {
                    if matches!(ch, '。' | '!' | '?' | '.' | '\n') {
                        short["text"] = json!(&body[..i + ch.len_utf8()]);
                        if !fits(&envelope(std::slice::from_ref(&short), start + 1)) {
                            break;
                        }
                        end = i + ch.len_utf8();
                    }
                }
                short["text"] = json!(&body[..end]);
                items.push(short);
            }
            break;
        }
    }
    Ok(envelope(&items, start + items.len()))
}

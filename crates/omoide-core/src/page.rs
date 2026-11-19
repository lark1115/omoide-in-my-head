use serde_json::{Value as Json, json};
pub fn page(
    cursor: Option<&str>,
    rev: i64,
    count: usize,
    capped: bool,
    results: &[Json],
) -> Result<Json, String> {
    let mut start = 0;
    let mut restarted = false;
    if let Some(token) = cursor {
        let (r, i) = token
            .split_once(':')
            .ok_or_else(|| "invalid cursor".to_owned())?;
        let r: i64 = r
            .parse()
            .map_err(|_| "invalid cursor revision".to_owned())?;
        let i: usize = i.parse().map_err(|_| "invalid cursor offset".to_owned())?;
        if rev == r {
            if i > results.len() {
                return Err("cursor out of range".to_owned());
            }
            start = i;
        } else {
            restarted = true
        }
    }
    let envelope = |items: &[Json], next: usize| {
        json!({
            "state": if results.is_empty() { "none" } else { "found" },
            "judge_state": "not_configured",
            "results": items,
            "candidate_count": count,
            "matched_count": results.len(),
            "candidates_truncated": capped,
            "more_search_available": capped,
            "remaining": results.len()-next,
            "cursor": (next<results.len()).then(||format!("{rev}:{next}")),
            "restarted": restarted,
        })
    };
    let mut items = Vec::new();
    for item in &results[start..] {
        items.push(item.clone());
        if envelope(&items, start + items.len()).to_string().len() + 1 > 4000 {
            items.pop();
            if items.is_empty() {
                let mut short = item.clone();
                short["text"] = json!("");
                short["truncated"] = json!(true);
                if envelope(&[short.clone()], start + 1).to_string().len() + 1 > 4000 {
                    return Err(
                        "rule conditions exceed recall output budget; retrieve by show".to_owned(),
                    );
                }
                let body = item["text"].as_str().unwrap_or_default();
                let mut end = 0;
                for (i, ch) in body.char_indices().take(4000) {
                    if matches!(ch, '。' | '!' | '?' | '.' | '\n') {
                        short["text"] = json!(&body[..i + ch.len_utf8()]);
                        if envelope(&[short.clone()], start + 1).to_string().len() + 1 > 4000 {
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

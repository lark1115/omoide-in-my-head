//! Short, explicitly unconfirmed records share the first-reply budget with confirmed matches.
use serde_json::{Value, json};

pub const GUIDANCE: &str = "Not confirmed to apply; read with show before using.";

pub fn summary(record: &Value) -> Value {
    let text = record["text"].as_str().unwrap_or_default().trim();
    let first = text
        .split_inclusive(['。', '.', '!', '?', '！', '？', '\n'])
        .next()
        .unwrap_or_default()
        .trim_end();
    json!({"id":record["id"],"version_id":record["version_id"],"kind":record["kind"],"target":record["target"],
           "actions":record["actions"],"scene":record["scene"],"text":first})
}

/// The caller has already packed the strict page with this envelope reserved.
pub fn append(mut page: Value, candidates: &[Value]) -> Value {
    for (i, candidate) in candidates.iter().enumerate() {
        page["candidates"]
            .as_array_mut()
            .unwrap()
            .push(summary(candidate));
        page["candidates_remaining"] = json!(candidates.len() - i - 1);
        if page.to_string().len() >= 4000 {
            page["candidates"].as_array_mut().unwrap().pop();
            page["candidates_remaining"] = json!(candidates.len() - i);
            break;
        }
    }
    page
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;
    fn row(text: &str) -> Value {
        json!({"id":"r","version_id":"v","kind":"rule","target":"box","actions":["pack"],"scene":"rain","text":text,"source":"private"})
    }
    // [REQ-RECALL-1]
    #[test]
    fn only_identifiers_trigger_and_first_sentence() {
        for (source, expected) in [
            (" 最初。続く。", "最初。"),
            ("Check now! Wait.", "Check now!"),
            ("Only one", "Only one"),
            ("", ""),
        ] {
            assert_eq!(
                summary(&row(source)),
                json!({"id":"r","version_id":"v","kind":"rule","target":"box","actions":["pack"],"scene":"rain","text":expected})
            );
        }
    }
    fn page() -> Value {
        json!({"results":[{"id":"confirmed"}],"candidates":[],"candidates_remaining":1})
    }
    // [REQ-RECALL-1]
    #[test]
    fn summaries_fit_exactly_with_newline_or_stay_unreturned() {
        let empty = append(page(), &[row("")]).to_string().len();
        let body = "a".repeat(3999 - empty);
        let exact = append(page(), &[row(&body)]);
        assert_eq!(exact.to_string().len() + 1, 4000);
        assert_eq!(exact["candidates_remaining"], 0);
        let overflow = append(page(), &[row(&(body + "b"))]);
        assert_eq!(overflow, page());
        let two = append(page(), &[row("one"), row("two")]);
        assert_eq!(two["candidates"][1]["text"], "two");
        assert_eq!(two["candidates_remaining"], 0);
    }
    proptest! {
        // [REQ-RECALL-1]
        #[test]
        fn strict_page_is_unchanged_and_every_omission_counted(n in 0usize..24,len in 0usize..180) {
            let rows=vec![row(&"文".repeat(len));n];
            let mut original=page();
            original["candidates_remaining"]=json!(n);
            let result=append(original.clone(),&rows);
            prop_assert_eq!(&result["results"],&original["results"]);
            prop_assert!(result.to_string().len()<4000);
            prop_assert_eq!(result["candidates"].as_array().unwrap().len()+result["candidates_remaining"].as_u64().unwrap() as usize,n);
        }
    }
}

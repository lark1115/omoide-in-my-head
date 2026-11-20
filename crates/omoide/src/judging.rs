//! Bound individual questions and requests before spawning an external judge.
use crate::{args::JudgeConfig, judge::exchange};
use omoide_core::judge::{Tiers, tiers};
use serde_json::{Value as Json, json};
use std::{
    collections::BTreeSet,
    time::{Duration, Instant},
};

pub struct Judgments {
    pub scores: Tiers,
    pub done: BTreeSet<String>,
    pub requested: usize,
    pub oversized: usize,
    pub error: Option<String>,
}
const REQUEST_BYTES: usize = 48_000;
const QUESTION_BYTES: usize = 24_000;
// Allow room for adapter-provided question text, criteria, field names and per-question framing.
const FRAMING: usize = 1024;

pub fn ask(config: &JudgeConfig, query: &Json, records: Vec<&Json>) -> Result<Judgments, String> {
    let start = Instant::now();
    let budget = Duration::from_millis(config.timeout_ms);
    let base = query.to_string().len() + FRAMING;
    let requested = records.len();
    let mut pending = Vec::new();
    let mut batches = Vec::new();
    let mut bytes = base;
    let mut error = None;
    let mut oversized = 0;
    for r in records {
        let c = json!({"version_id":r["version_id"],"kind":r["kind"],"text":r["text"],"scene":r["scene"],"aliases":r["aliases"]});
        let size = c.to_string().len() + FRAMING;
        if base + size > QUESTION_BYTES {
            oversized += 1;
            error = Some("judge question exceeds byte budget".into());
            continue;
        }
        if bytes + size > REQUEST_BYTES {
            batches.push(std::mem::take(&mut pending));
            bytes = base;
        }
        bytes += size;
        pending.push(c);
    }
    if !pending.is_empty() || requested == 0 {
        batches.push(pending);
    }
    let mut answers = vec![];
    let mut answered = BTreeSet::new();
    let mut done = BTreeSet::new();
    let mut remaining_output = config.output_bytes;
    for batch in batches {
        let ids = batch
            .iter()
            .map(|c| c["version_id"].as_str().unwrap().to_owned())
            .collect::<BTreeSet<_>>();
        let outcome = exchange(
            config,
            &json!({"query":query,"candidates":batch}),
            start,
            budget,
            remaining_output,
        )
        .and_then(|raw| {
            remaining_output -= raw.len();
            tiers(&raw, &ids, config.threshold)?;
            let reply: Json = serde_json::from_slice(&raw).map_err(|e| e.to_string())?;
            Ok(reply["answers"].as_array().unwrap().clone())
        });
        match outcome {
            Ok(v) => {
                if !v.is_empty() {
                    answered.extend(ids.iter().cloned());
                    answers.extend(v)
                }
                done.extend(ids);
            }
            Err(reason) => {
                error = Some(reason);
                break;
            }
        }
    }
    let merged = json!({"answers":answers}).to_string();
    Ok(Judgments {
        scores: tiers(merged.as_bytes(), &answered, config.threshold)?,
        done,
        requested,
        oversized,
        error,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{fs, path::PathBuf};
    fn record(id: usize, len: usize) -> Json {
        json!({"version_id":id.to_string(),"kind":"fact","text":"a".repeat(len),"scene":null,"aliases":null})
    }
    // [REQ-JUDGE-1]
    #[test]
    fn bounded_batches_keep_cross_batch_order_and_only_complete_answers() {
        let dir = tempfile::tempdir().unwrap();
        let script = dir.path().join("score.py");
        fs::write(&script,r#"import json,sys
r=json.load(sys.stdin); c=r['candidates']
assert all(v['kind']=='fact' for v in c) and 1 <= len(c) <= 2
if sys.argv[1]=='stop' and any(v['version_id']=='2' for v in c): sys.exit(124)
print(json.dumps({'answers':[{'version_id':v['version_id'],'score':0.61+int(v['version_id'])*0.05} for v in c]}))
"#).unwrap();
        let rows = [
            record(0, 18000),
            record(1, 18000),
            record(2, 18000),
            record(3, 18000),
        ];
        for mode in ["full", "stop"] {
            let config = JudgeConfig {
                judge: Some(PathBuf::from("python3")),
                args: vec![script.to_string_lossy().into_owned(), mode.into()],
                timeout_ms: 5000,
                output_bytes: 20000,
                threshold: 0.6,
            };
            let result = ask(&config, &json!({"scene":"packing"}), rows.iter().collect()).unwrap();
            assert_eq!(result.requested, 4);
            if mode == "full" {
                assert_eq!(result.done.len(), 4);
                assert_eq!(result.error, None);
                assert!(result.scores.confirmed["2"] > result.scores.confirmed["1"]);
            } else {
                assert_eq!(result.done, BTreeSet::from(["0".into(), "1".into()]));
                assert_eq!(result.error.as_deref(), Some("judge timeout"));
                assert!(!result.scores.confirmed.contains_key("2"));
            }
        }
    }
    // [REQ-JUDGE-1]
    #[test]
    fn oversized_question_never_starts_a_judge() {
        let cfg = JudgeConfig {
            judge: None,
            args: vec![],
            timeout_ms: 100,
            output_bytes: 100,
            threshold: 0.6,
        };
        let r = record(0, 25000);
        let result = ask(&cfg, &json!({}), vec![&r]).unwrap();
        assert_eq!(
            result.error.as_deref(),
            Some("judge question exceeds byte budget")
        );
        assert!(result.done.is_empty());
        assert_eq!(result.oversized, 1);
    }
    // [REQ-JUDGE-1]
    #[test]
    fn honor_both_inclusive_input_bounds_and_one_output_limit() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("echo.py");
        let log = dir.path().join("lengths");
        fs::write(&path,"import json,sys\nr=json.load(sys.stdin)\nwith open(sys.argv[1],'a') as f: print(len(r['candidates']),file=f)\nprint('{\"answers\":[]}')").unwrap();
        let mut cfg = JudgeConfig {
            judge: Some(PathBuf::from("python3")),
            args: vec![
                path.to_string_lossy().into_owned(),
                log.to_string_lossy().into_owned(),
            ],
            timeout_ms: 5000,
            output_bytes: 500,
            threshold: 0.6,
        };
        let base = 1024 + 2; // serialized empty query, plus framing
        let fixed = 1024 + record(0, 0).to_string().len();
        let exact = record(0, 24000 - base - fixed);
        assert_eq!(ask(&cfg, &json!({}), vec![&exact]).unwrap().done.len(), 1);
        let over = record(0, 24001 - base - fixed);
        assert_eq!(ask(&cfg, &json!({}), vec![&over]).unwrap().oversized, 1);
        let available = 48000 - base - 3 * fixed;
        for (extra, expected) in [(0, "3\n"), (1, "2\n1\n")] {
            fs::write(&log, "").unwrap();
            let rows = [
                record(0, available / 3),
                record(1, available / 3),
                record(2, available - 2 * (available / 3) + extra),
            ];
            assert!(
                ask(&cfg, &json!({}), rows.iter().collect())
                    .unwrap()
                    .error
                    .is_none()
            );
            assert_eq!(fs::read_to_string(&log).unwrap(), expected);
        }
        let rows = [
            record(0, 18000),
            record(1, 18000),
            record(2, 18000),
            record(3, 18000),
        ];
        let reply_bytes = b"{\"answers\":[]}\n".len();
        for (cap, expected, error) in [
            (2 * reply_bytes, 4, None),
            (2 * reply_bytes - 1, 2, Some("judge output limit")),
        ] {
            cfg.output_bytes = cap;
            let result = ask(&cfg, &json!({}), rows.iter().collect()).unwrap();
            assert_eq!(result.done.len(), expected);
            assert_eq!(result.error.as_deref(), error);
        }
    }
}

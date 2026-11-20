use super::*;
use protocol::MODEL;
const RECORDED: &[u8] = include_bytes!("../tests/fixtures/s1-response.json");
fn input(ids: &[&str]) -> Input {
    serde_json::from_value(json!({
        "query":{"scene":"packing","action":"wrap","target":"studio","private":"omit"},
        "candidates":ids.iter().map(|id|json!({"version_id":id,"text":"Wrap with paper","scene":"packing","aliases":["parcel"],"source_text":"omit"})).collect::<Vec<_>>()
    })).unwrap()
}
// [REQ-JUDGE-1]
#[test]
fn one_question_per_rule_with_only_allowlisted_content() {
    let payload = input(&["local-first", "local-second"]).request().unwrap();
    assert_eq!(payload["model"], "jev-1.14.0");
    assert_eq!(
        payload["state"],
        json!({"scene":"packing","action":"wrap","target":"studio"})
    );
    assert_eq!(payload.as_object().unwrap().len(), 3);
    assert_eq!(payload["questions"].as_object().unwrap().len(), 2);
    for name in ["q0", "q1"] {
        let q = &payload["questions"][name];
        assert_eq!(q["type"], "noul");
        assert_eq!(
            q["instructions"]["rule"],
            json!({"text":"Wrap with paper","scene":"packing","aliases":["parcel"]})
        );
        assert_eq!(
            q["instructions"]["question"],
            "Does this rule apply to the situation in the shared state?"
        );
        assert!(
            q["criteria"]["true"]
                .as_str()
                .unwrap()
                .starts_with("The situation calls for")
        );
        assert!(
            q["criteria"]["false"]
                .as_str()
                .unwrap()
                .starts_with("The situation concerns a different task")
        );
    }
    assert!(!payload.to_string().contains("local-"));
    assert!(!payload.to_string().contains("omit"));
    assert!(input(&["a", "a"]).request().is_err());
    assert!(input(&[""]).request().is_err());
}
// [REQ-JUDGE-1]
#[test]
fn recorded_probability_and_usage_map_to_the_local_version() {
    let (reply, usage) = input(&["local"]).decode(RECORDED).unwrap();
    assert_eq!(
        reply,
        json!({"answers":[{"version_id":"local","score":0.97}]})
    );
    assert_eq!(
        usage,
        Usage {
            input_tokens: 417,
            output_tokens: 21
        }
    );
}
// [REQ-JUDGE-1]
#[test]
fn require_complete_typed_probabilities_and_match_names_not_response_order() {
    let pair = input(&["a", "b"]);
    let mut r = json!({"model":MODEL,"usage":{"input_tokens":14,"output_tokens":10},"answers":{
        "q1":{"type":"noul","noul":1,"confidence":0.03},"q0":{"type":"noul","noul":0,"confidence":0.99}
    }});
    let decode = |v: &Value| pair.decode(&serde_json::to_vec(v).unwrap());
    assert_eq!(
        decode(&r).unwrap().0,
        json!({"answers":[{"version_id":"a","score":0.0},{"version_id":"b","score":1.0}]})
    );
    for (field, bad) in [
        ("type", json!("choice")),
        ("noul", json!(-0.01)),
        ("noul", json!(1.01)),
        ("noul", json!("0.5")),
        ("noul", Value::Null),
    ] {
        let mut copy = r.clone();
        copy["answers"]["q0"][field] = bad;
        assert!(decode(&copy).is_err());
    }
    let mut copy = r.clone();
    copy["answers"]["q2"] = r["answers"]["q1"].clone();
    assert!(decode(&copy).is_err());
    for field in ["model", "usage", "answers"] {
        let mut copy = r.clone();
        copy.as_object_mut().unwrap().remove(field);
        assert!(decode(&copy).is_err());
    }
    let mut copy = r.clone();
    copy["model"] = json!("jev-latest");
    assert!(decode(&copy).is_err());
    let mut copy = r.clone();
    copy["usage"]["input_tokens"] = json!(-1);
    assert!(decode(&copy).is_err());
    r["answers"].as_object_mut().unwrap().remove("q1");
    assert!(decode(&r).is_err());
    let duplicate = String::from_utf8(RECORDED.to_vec())
        .unwrap()
        .replace("\"q0\":", "\"q0\":{\"type\":\"noul\",\"noul\":0.1},\"q0\":");
    assert!(input(&["one"]).decode(duplicate.as_bytes()).is_err());
    assert!(pair.decode(b"garbage").is_err());
}
// [REQ-JUDGE-1]
#[test]
fn http_contract_and_failures_without_network() {
    let budget = Duration::from_millis(128);
    let call = |status: u16| {
        post_with(
            &json!({"synthetic":true}),
            "in-memory-test-marker",
            budget,
            |cfg, req| {
                assert_eq!(req.method(), "POST");
                assert_eq!(
                    req.uri().to_string(),
                    "https://api.typesafe.ai/v1/systemone"
                );
                assert!(cfg.https_only());
                assert_eq!(cfg.max_redirects(), 0);
                assert_eq!(cfg.timeouts().global, Some(budget));
                assert!(req.headers()["Authorization"].is_sensitive());
                assert!(req.headers()["Authorization"] == "Bearer in-memory-test-marker");
                assert_eq!(req.headers()["Content-Type"], "application/json");
                assert_eq!(
                    serde_json::from_slice::<Value>(req.body()).unwrap(),
                    json!({"synthetic":true})
                );
                Ok(Response::builder()
                    .status(status)
                    .body(ureq::Body::builder().data(RECORDED))
                    .unwrap())
            },
        )
    };
    assert_eq!(call(200).unwrap(), RECORDED);
    for status in [301, 401, 429, 503] {
        assert_eq!(call(status).unwrap_err(), format!("TypeSafe HTTP {status}"));
    }
    for (error, message) in [
        (ureq::Error::StatusCode(403), "TypeSafe HTTP 403"),
        (
            ureq::Error::Timeout(ureq::Timeout::Global),
            "TypeSafe request timed out",
        ),
        (
            ureq::Error::Io(io::Error::other("internal detail")),
            "TypeSafe transport failed",
        ),
    ] {
        assert_eq!(
            post_with(&Value::Null, "marker", budget, |_, _| Err(error)).unwrap_err(),
            message
        );
    }
}
// [REQ-JUDGE-1]
#[test]
fn deadline_and_all_or_nothing_dispatch() {
    let budget = Duration::from_secs(2);
    for elapsed in [budget, budget + Duration::from_nanos(1)] {
        assert!(remaining(budget, elapsed).is_err());
    }
    assert_eq!(
        remaining(budget, budget - Duration::from_nanos(1)),
        Ok(Duration::from_nanos(1))
    );
    for key in [None, Some(" \t")] {
        assert!(
            evaluate(
                &input(&["a"]),
                key,
                budget,
                Instant::now(),
                |_, _, _| panic!("unexpected transport")
            )
            .is_err()
        );
    }
    let (r, u) = evaluate(
        &input(&[]),
        Some("marker"),
        budget,
        Instant::now(),
        |_, _, _| panic!("unexpected transport"),
    )
    .unwrap();
    assert_eq!(r, json!({"answers":[]}));
    assert!(u.is_none());
    assert!(
        evaluate(
            &input(&["a"]),
            Some("marker"),
            Duration::ZERO,
            Instant::now(),
            |_, _, _| panic!("unexpected transport")
        )
        .is_err()
    );
    let eval = |bytes: &[u8]| {
        evaluate(
            &input(&["a"]),
            Some("marker"),
            budget,
            Instant::now(),
            |_, _, timeout| {
                assert!(timeout <= budget);
                Ok(bytes.to_vec())
            },
        )
    };
    assert_eq!(eval(RECORDED).unwrap().0["answers"][0]["score"], 0.97);
    assert!(eval(b"{}").is_err());
    assert!(
        evaluate(
            &input(&["a"]),
            Some("marker"),
            budget,
            Instant::now(),
            |_, _, _| Err("transport failed".into())
        )
        .is_err()
    );
}

// [REQ-JUDGE-1]
#[test]
fn unrelated_recorded_case_is_not_rounded_to_yes_or_no() {
    let bytes = include_bytes!("../tests/fixtures/s1-negative-response.json");
    let raw: Value = serde_json::from_slice(bytes).unwrap();
    let (response, usage) = input(&["negative"]).decode(bytes).unwrap();
    assert_eq!(
        response["answers"][0]["score"],
        raw["answers"]["q0"]["noul"]
    );
    assert!(response["answers"][0]["score"].as_f64().unwrap() < 0.3);
    assert_eq!(json!(usage), raw["usage"]);
}

// [REQ-JUDGE-1]
#[test]
fn distinct_candidate_content_is_bound_to_each_question_in_one_exchange() {
    let input:Input=serde_json::from_value(json!({
        "query":{"action":"prepare"},
        "candidates":[{"version_id":"first","text":"Wrap paper","scene":"shipping","aliases":["dispatch"]},
                      {"version_id":"second","text":"Count boxes","scene":"inventory","aliases":null}]
    })).unwrap();
    let result=evaluate(&input,Some("marker"),Duration::from_secs(2),Instant::now(),|payload,_,_|{
        let questions=&payload["questions"];
        assert_eq!(questions["q0"]["instructions"]["rule"],json!({"text":"Wrap paper","scene":"shipping","aliases":["dispatch"]}));
        assert_eq!(questions["q1"]["instructions"]["rule"],json!({"text":"Count boxes","scene":"inventory","aliases":null}));
        Ok(json!({"model":MODEL,"answers":{"q1":{"type":"noul","noul":0.85},"q0":{"type":"noul","noul":0.15}},
            "usage":{"input_tokens":200,"output_tokens":30}}).to_string().into_bytes())
    }).unwrap();
    assert_eq!(
        result.0,
        json!({"answers":[{"version_id":"first","score":0.15},{"version_id":"second","score":0.85}]})
    );
}

// [REQ-JUDGE-1]
#[test]
fn choose_question_by_kind_and_refuse_oversize_before_transport() {
    let make = |kind: &str, text: &str| json!({"version_id":kind,"kind":kind,"text":text});
    let build = |items: Vec<Value>| {
        serde_json::from_value::<Input>(json!({"query":{},"candidates":items}))
            .unwrap()
            .request()
    };
    let kinds = ["rule", "skill", "decision", "fact", "experience"];
    let payload = build(kinds.iter().map(|k| make(k, "invented")).collect()).unwrap();
    let q = |i| {
        payload["questions"][format!("q{i}")]["instructions"]["question"]
            .as_str()
            .unwrap()
    };
    assert_eq!(q(0), q(1));
    assert!(q(2).contains("decision"));
    assert!(q(3).contains("factual"));
    assert!(q(4).contains("observation"));
    assert_eq!(
        build(vec![make("fact", &"紙".repeat(9000))]).unwrap_err(),
        "question exceeds byte budget"
    );
    assert_eq!(
        build(kinds.iter().map(|k| make(k, &"a".repeat(10000))).collect()).unwrap_err(),
        "request exceeds byte budget"
    );
}

// [REQ-JUDGE-1]
#[test]
fn serialized_question_and_batch_fit_at_exact_byte_boundaries() {
    let build = |texts: Vec<String>| {
        let candidates: Vec<_> = texts
            .into_iter()
            .enumerate()
            .map(|(i, t)| json!({"version_id":i.to_string(),"text":t}))
            .collect();
        serde_json::from_value::<Input>(json!({"query":{},"candidates":candidates}))
            .unwrap()
            .request()
    };
    let sample = build(vec![String::new()]).unwrap();
    let fixed = sample["state"].to_string().len() + sample["questions"]["q0"].to_string().len();
    let at = 24000 - fixed;
    assert!(build(vec!["x".repeat(at)]).is_ok());
    assert_eq!(
        build(vec!["x".repeat(at + 1)]).unwrap_err(),
        "question exceeds byte budget"
    );
    let fixed = build(vec![String::new(); 3]).unwrap().to_string().len();
    let available = 48000 - fixed;
    let widths = vec![
        available / 3,
        available / 3,
        available - 2 * (available / 3),
    ];
    let mut texts: Vec<_> = widths.into_iter().map(|n| "a".repeat(n)).collect();
    assert_eq!(build(texts.clone()).unwrap().to_string().len(), 48000);
    texts[2].push('b');
    assert_eq!(build(texts).unwrap_err(), "request exceeds byte budget");
}

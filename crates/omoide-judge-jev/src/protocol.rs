//! Only the shared situation and per-question rule fields cross the service boundary.
use serde::{
    Deserialize, Deserializer, Serialize,
    de::{Error as _, MapAccess, Visitor},
};
use serde_json::{Value, json};
use std::{
    collections::{BTreeMap, BTreeSet},
    fmt,
};

pub const MODEL: &str = "jev-1.14.0";
const QUESTION: &str = "Does this rule apply to the situation in the shared state?";
const YES: &str =
    "The situation calls for the procedure described by the rule, even when phrased differently.";
const NO: &str =
    "The situation concerns a different task or falls outside the rule's stated circumstances.";

#[derive(Deserialize)]
pub struct Input {
    query: Situation,
    candidates: Vec<Candidate>,
}
#[derive(Serialize, Deserialize)]
struct Situation {
    scene: Option<String>,
    action: Option<String>,
    target: Option<String>,
}
#[derive(Deserialize)]
struct Candidate {
    kind: Option<String>,
    version_id: String,
    text: String,
    scene: Option<String>,
    aliases: Option<Vec<String>>,
}
impl Input {
    pub fn is_empty(&self) -> bool {
        self.candidates.is_empty()
    }
    pub fn request(&self) -> Result<Value, &'static str> {
        let mut versions = BTreeSet::new();
        let mut questions = BTreeMap::new();
        for (index, c) in self.candidates.iter().enumerate() {
            if c.version_id.is_empty() || !versions.insert(&c.version_id) {
                return Err("invalid candidate IDs");
            }
            let (question, yes, no) = match c.kind.as_deref() {
                Some("decision") => (
                    "Does this decision concern the choice described?",
                    "The recorded choice informs this situation, including a pending choice.",
                    "The subject or choice differs.",
                ),
                Some("fact") => (
                    "Is this factual record relevant evidence for this situation?",
                    "The fact bears on the subject and circumstances, at its stated certainty.",
                    "The fact concerns another subject or circumstances.",
                ),
                Some("experience") => (
                    "Does this observation document relevant work for this situation?",
                    "The observed work or outcome informs the situation within its scope and time.",
                    "The observation concerns different work, scope or time.",
                ),
                _ => (QUESTION, YES, NO),
            };
            questions.insert(format!("q{index}"),json!({"type":"noul","instructions":{"question":question,
                "rule":{"text":c.text,"scene":c.scene,"aliases":c.aliases}},"criteria":{"true":yes,"false":no}}));
        }
        let state_bytes = serde_json::to_vec(&self.query).unwrap().len();
        if questions
            .values()
            .any(|q| state_bytes + q.to_string().len() > 24_000)
        {
            return Err("question exceeds byte budget");
        }
        let body = json!({"model":MODEL,"state":self.query,"questions":questions});
        if body.to_string().len() > 48_000 {
            return Err("request exceeds byte budget");
        }
        Ok(body)
    }

    pub fn decode(&self, bytes: &[u8]) -> Result<(Value, Usage), &'static str> {
        let response: Response =
            serde_json::from_slice(bytes).map_err(|_| "invalid TypeSafe response")?;
        if response.model != MODEL {
            return Err("unexpected model");
        }
        let mut answers = response.answers;
        let mut result = Vec::with_capacity(self.candidates.len());
        for (index, c) in self.candidates.iter().enumerate() {
            let a = answers
                .remove(&format!("q{index}"))
                .ok_or("missing Noul answer")?;
            if !(0.0..=1.0).contains(&a.noul) {
                return Err("invalid Noul probability");
            }
            result.push(json!({"version_id":c.version_id,"score":a.noul}));
        }
        if !answers.is_empty() {
            return Err("unexpected answer ID");
        }
        Ok((json!({"answers":result}), response.usage))
    }
}

#[derive(Deserialize)]
struct Response {
    model: String,
    #[serde(deserialize_with = "unique_answers")]
    answers: BTreeMap<String, Noul>,
    usage: Usage,
}
#[derive(Deserialize)]
struct Noul {
    #[serde(rename = "type")]
    _kind: NoulKind,
    noul: f64,
}
#[derive(Deserialize)]
enum NoulKind {
    #[serde(rename = "noul")]
    Noul,
}
#[derive(Debug, Serialize, Deserialize, PartialEq)]
pub struct Usage {
    pub input_tokens: u64,
    pub output_tokens: u64,
}

fn unique_answers<'de, D: Deserializer<'de>>(d: D) -> Result<BTreeMap<String, Noul>, D::Error> {
    struct Unique;
    impl<'de> Visitor<'de> for Unique {
        type Value = BTreeMap<String, Noul>;
        fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
            f.write_str("unique question names")
        }
        fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Self::Value, A::Error> {
            let mut values = BTreeMap::new();
            while let Some((key, val)) = map.next_entry()? {
                if values.insert(key, val).is_some() {
                    return Err(A::Error::custom("duplicate answer"));
                }
            }
            Ok(values)
        }
    }
    d.deserialize_map(Unique)
}

#[cfg(test)]
mod wire_tests {
    use super::Response;
    // [REQ-JUDGE-1]
    #[test]
    fn answers_must_be_a_named_object_not_an_array() {
        let raw =
            br#"{"model":"jev-1.14.0","answers":[],"usage":{"input_tokens":0,"output_tokens":0}}"#;
        let error = serde_json::from_slice::<Response>(raw)
            .err()
            .expect("array must be rejected");
        assert!(error.to_string().contains("expected unique question names"));
    }
}

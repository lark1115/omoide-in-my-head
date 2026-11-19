//! Build one immutable evidence record and its lexical index entry in the same batch.
use crate::{
    args::{Command, EventCommand, FactCommand},
    db::*,
    write::nonempty,
};
use omoide_store::{Error, Rows, Statement, Value};
use serde_json::{Value as Json, json};

pub fn insert(
    cmd: &Command,
    (new_id, new_version): (&str, &str),
    time: i64,
    rows: &[Rows],
    op: &str,
) -> Result<(Vec<Statement>, Json)> {
    let origin = rows.first().map(required_record).transpose()?;
    let mut changes = Vec::new();
    let mut id = new_id;
    let mut version = new_id;
    let (kind, body, target, created, details) = match cmd {
        Command::Decide(d) => {
            version = new_version;
            if let Some(old) = &d.revises {
                let previous = rows[1]
                    .first()
                    .ok_or_else(|| Error::Input("decision version is not current".into()))?;
                id = super::db::text(&previous[0])?;
                changes.push(q("UPDATE memories SET details=json_set(details,'$.decision','revised','$.successor',?) WHERE id=?",vec![s(version),s(old)]));
            }
            let u = origin.as_ref().unwrap();
            if u["kind"] != "utterance" || u["speaker"] != "user" {
                return Err(Error::Input(
                    "decision requires the user's utterance".into(),
                ));
            }
            (
                "decision",
                d.text.as_str(),
                &d.target,
                d.at.unwrap_or(time),
                json!({"speech":d.speech,"decision":d.state,"scope":d.scope,"period":d.period}),
            )
        }
        Command::Event {
            command:
                EventCommand::Add {
                    text,
                    at,
                    target,
                    scope,
                    period,
                },
        } => (
            "experience",
            text.as_str(),
            target,
            at.unwrap_or(time),
            json!({"scope":scope,"period":period}),
        ),
        Command::Fact {
            command:
                FactCommand::Add {
                    text,
                    target,
                    certainty,
                    attributes: a,
                    ..
                },
        } => {
            let mut details = json!(a);
            let u = origin.as_ref().unwrap();
            let verified = matches!(certainty.as_str(), "verified" | "self-verified");
            if verified {
                // An observed event supplies the observation time, context and verbatim method.
                let observed = u["kind"] == "experience";
                let where_ = a
                    .verified_where
                    .as_deref()
                    .or(a.location.as_deref())
                    .or(if observed { u["target"].as_str() } else { None });
                let when = a.verified_when.or(if observed {
                    u["created"].as_i64()
                } else {
                    None
                });
                let how =
                    a.verified_how
                        .as_deref()
                        .or(if observed { u["text"].as_str() } else { None });
                let evidence =
                    a.evidence
                        .as_deref()
                        .or(if observed { u["id"].as_str() } else { None });
                if where_.is_none_or(|v| v.trim().is_empty())
                    || when.is_none()
                    || how.is_none_or(|v| v.trim().is_empty())
                    || evidence.is_none_or(|v| v.trim().is_empty())
                {
                    return Err(Error::Input(
                        "verified fact requires observation place, time, method and evidence"
                            .into(),
                    ));
                }
                details["verified_where"] = json!(where_);
                details["verified_when"] = json!(when);
                details["verified_how"] = json!(how);
                details["evidence"] = json!(evidence);
            }
            details["certainty"] = json!(if verified { "verified" } else { certainty });
            ("fact", text.as_str(), target, a.at.unwrap_or(time), details)
        }
        _ => unreachable!(),
    };
    nonempty(body)?;
    let origin_id = origin
        .as_ref()
        .and_then(|o| o["id"].as_str())
        .map_or(Value::Null, s);
    let insert = q(
        "INSERT INTO memories(id,record_id,kind,body,target,created,source,details) VALUES(?,?,?,?,?,?,?,?)",
        vec![
            s(version),
            s(id),
            s(kind),
            s(body),
            opt(target),
            Value::Integer(created),
            origin_id,
            s(&details.to_string()),
        ],
    );
    let index = omoide_store::evidence::index(version, body, target.as_deref());
    changes.extend([insert, index]);
    Ok((
        changes,
        json!({"id":id,"version_id":version,"state":"stored","kind":kind,"operation_id":op}),
    ))
}

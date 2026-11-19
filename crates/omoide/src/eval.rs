//! Offline evaluation. The adapter runs every CLI call in a new process with a scratch database.
use crate::{args::EvalCommand, db::*};
use omoide_store::Error;
use serde::Deserialize;
use serde_json::{Value as Json, json};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::Path,
    process::Command,
};

fn invalid(e: impl std::fmt::Display) -> Error {
    Error::Input(e.to_string())
}
fn load(path: &Path) -> Result<Json> {
    serde_json::from_slice(&fs::read(path).map_err(invalid)?).map_err(invalid)
}
struct Study<'a> {
    scenes: &'a Path,
    scene: Option<&'a str>,
    judge: Option<&'a Path>,
    aliases: Option<&'a Path>,
    oracle: bool,
}
impl Study<'_> {
    fn execute(
        &self,
        binary: &Path,
        fault: Option<&Path>,
        comparison: Option<(&Path, Option<&Path>)>,
    ) -> Result<Json> {
        let dir = tempfile::tempdir().map_err(invalid)?;
        fs::write(
            dir.path().join("scene_adapter.py"),
            include_str!("../support/scene_adapter.py"),
        )
        .map_err(invalid)?;
        fs::write(
            dir.path().join("interruption.py"),
            include_str!("../support/interruption.py"),
        )
        .map_err(invalid)?;
        let mut cmd = Command::new("python3");
        cmd.args(["-I","-c","import runpy,sys; sys.path.insert(0,sys.argv.pop(1)); runpy.run_module('scene_adapter',run_name='__main__')"])
            .arg(dir.path()).arg(binary).args(["--json","--stage","4","--scenes"]).arg(self.scenes);
        if let Some(f) = fault {
            cmd.arg("--fault-binary").arg(f);
        }
        if let Some(j) = self.judge {
            cmd.arg("--judge-fixture").arg(j);
        }
        if let Some(a) = self.aliases {
            cmd.arg("--aliases").arg(a);
        }
        if let Some(s) = self.scene {
            cmd.arg("--scene").arg(s);
        }
        if self.oracle {
            if self.scene != Some("S1") {
                return Err(invalid("oracle requires --scene S1"));
            }
            cmd.arg("--oracle-table")
                .arg(self.scenes.join("../support/s1-oracle.json"));
        }
        if let Some((after, after_fault)) = comparison {
            cmd.arg("--compare-binary").arg(after);
            if let Some(f) = after_fault {
                cmd.arg("--compare-fault").arg(f);
            }
        }
        let output = cmd.output().map_err(invalid)?;
        if !output.status.success() {
            return Err(invalid(format!(
                "evaluation failed: {}",
                String::from_utf8_lossy(&output.stderr)
            )));
        }
        serde_json::from_slice(&output.stdout).map_err(invalid)
    }
}

#[derive(Deserialize)]
struct Trial {
    opportunity: String,
    variant: String,
    called: bool,
    delivered_at: Option<u64>,
    acted_at: Option<u64>,
    used: Option<bool>,
    action: Option<String>,
    corrected: Option<bool>,
    added_input: String,
}
fn trial_report(input: Json) -> Result<Json> {
    let trials: Vec<Trial> = serde_json::from_value(input).map_err(invalid)?;
    let mut seen = BTreeSet::new();
    let mut totals = BTreeMap::<String, (usize, usize, usize)>::new();
    let mut rows = Vec::new();
    for t in trials {
        if !matches!(t.variant.as_str(), "device" | "no_device" | "hot_only")
            || t.opportunity.trim().is_empty()
            || !seen.insert((t.variant.clone(), t.opportunity.clone()))
        {
            return Err(invalid("invalid or duplicate trial opportunity"));
        }
        if (t.variant == "no_device"
            && (t.called || !t.added_input.is_empty() || t.used == Some(true)))
            || (t.variant == "hot_only" && t.called)
        {
            return Err(invalid("trial observations conflict with assigned variant"));
        }
        let timing = match (t.delivered_at, t.acted_at) {
            (Some(a), Some(b)) if a < b => "before",
            (Some(a), Some(b)) if a > b => "after",
            _ => "unknown",
        };
        let counts = totals.entry(t.variant.clone()).or_default();
        counts.0 += 1;
        let timely = timing == "before" && t.used == Some(true);
        counts.1 += usize::from(timely);
        counts.2 += t.added_input.len();
        rows.push(json!({"opportunity":t.opportunity,"variant":t.variant,"called":t.called,"timing":timing,"delivered_at":t.delivered_at,"acted_at":t.acted_at,
            "used":t.used,"action":t.action,"corrected":t.corrected,"added_bytes":t.added_input.len()}));
    }
    let groups: Vec<_> = totals
        .iter()
        .map(
            |(v, (n, h, b))| json!({"variant":v,"opportunities":n,"timely_uses":h,"added_bytes":b}),
        )
        .collect();
    Ok(json!({"trials":rows,"groups":groups}))
}

fn cases(report: &Json) -> Result<BTreeMap<&str, &Json>> {
    let mut found = BTreeMap::new();
    for row in report["cases"]
        .as_array()
        .ok_or_else(|| invalid("missing case array"))?
    {
        let id = row["case"]
            .as_str()
            .ok_or_else(|| invalid("missing case id"))?;
        if found.insert(id, row).is_some() {
            return Err(invalid("duplicate case id"));
        }
    }
    Ok(found)
}
fn diff(before: &Json, after: &Json, allow: &[Json]) -> Result<Json> {
    let a = cases(before)?;
    let b = cases(after)?;
    let mut allowed = Vec::new();
    let mut unexplained = Vec::new();
    for id in a.keys().chain(b.keys()).copied().collect::<BTreeSet<_>>() {
        let prev = a.get(id).copied().unwrap_or(&Json::Null);
        let next = b.get(id).copied().unwrap_or(&Json::Null);
        let fields = prev
            .as_object()
            .into_iter()
            .chain(next.as_object())
            .flat_map(|o| o.keys())
            .collect::<BTreeSet<_>>();
        for key in fields {
            if prev.get(key) == next.get(key) {
                continue;
            }
            let mut change = json!({"case":id,"field":key,"before":prev[key],"after":next[key]});
            if prev.get(key).is_none() || next.get(key).is_none() {
                change["before_present"] = json!(prev.get(key).is_some());
                change["after_present"] = json!(next.get(key).is_some());
            }
            if allow.contains(&change) {
                allowed.push(change)
            } else {
                unexplained.push(change)
            }
        }
    }
    for key in [
        "dataset_digest",
        "settings_digest",
        "variant",
        "observations",
    ] {
        if before.get(key) != after.get(key) {
            let change =
                json!({"case":"$study","field":key,"before":before[key],"after":after[key]});
            if allow.contains(&change) {
                allowed.push(change)
            } else {
                unexplained.push(change)
            }
        }
    }
    Ok(
        json!({"state":if unexplained.is_empty(){"clean"}else{"differences"},"allowed":allowed,"unexplained":unexplained}),
    )
}
pub fn run(command: &EvalCommand) -> Result<Json> {
    match command {
        EvalCommand::Run {
            scenes,
            binary,
            fault_binary,
            judge_fixture,
            aliases,
            oracle,
            scene,
            trials,
        } => {
            let study = Study {
                scenes,
                scene: scene.as_deref(),
                judge: judge_fixture.as_deref(),
                aliases: aliases.as_deref(),
                oracle: *oracle,
            };
            let exe = std::env::current_exe().map_err(invalid)?;
            let mut result = study.execute(
                binary.as_deref().unwrap_or(&exe),
                fault_binary.as_deref(),
                None,
            )?;
            if let Some(t) = trials {
                result["observations"] = trial_report(load(t)?)?;
            }
            Ok(result)
        }
        EvalCommand::Diff {
            before,
            after,
            allow,
            builds,
            scenes,
            scene,
            before_fault,
            after_fault,
        } => {
            let study = Study {
                scenes,
                scene: scene.as_deref(),
                judge: None,
                aliases: None,
                oracle: false,
            };
            let (a, b) = if *builds {
                let pair = study.execute(
                    before,
                    before_fault.as_deref(),
                    Some((after, after_fault.as_deref())),
                )?;
                (pair["before"].clone(), pair["after"].clone())
            } else {
                (load(before)?, load(after)?)
            };
            let allow = allow
                .as_deref()
                .map(load)
                .transpose()?
                .unwrap_or_else(|| json!([]));
            diff(
                &a,
                &b,
                allow
                    .as_array()
                    .ok_or_else(|| invalid("allowed differences must be an array"))?,
            )
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    // [REQ-MEASURE-1]
    #[test]
    fn trials_count_uncalled_opportunities_and_unknown_order_without_credit() {
        let sample = |variant, called, time, used, text| json!({"opportunity":"test","variant":variant,"called":called,"delivered_at":time,"acted_at":2,"used":used,"action":"move","corrected":false,"added_input":text});
        let device = sample("device", true, Some(1), true, "紙");
        let no = sample("no_device", false, None, false, "");
        let hot = sample("hot_only", false, Some(2), true, "節");
        let r = trial_report(json!([device, no, hot])).unwrap();
        assert_eq!(r["trials"][0]["added_bytes"], 3);
        assert_eq!(r["trials"][1]["timing"], "unknown");
        assert_eq!(r["trials"][2]["timing"], "unknown");
        assert_eq!(r["groups"][0]["timely_uses"], 1);
        assert_eq!(r["groups"][0]["added_bytes"], 3);
        assert_eq!(r["groups"][1]["timely_uses"], 0);
        assert_eq!(r["groups"][2]["opportunities"], 1);
        assert_eq!(
            trial_report(json!([sample("device", true, Some(3), true, "")])).unwrap()["trials"][0]
                ["timing"],
            "after"
        );
        for bad in [
            json!([device, device]),
            json!([sample("other", false, None, false, "")]),
            json!([sample("hot_only", true, None, false, "")]),
            json!([sample("no_device", false, None, true, "")]),
            json!([sample("no_device", false, None, false, "data")]),
            json!([sample("no_device", true, None, false, "")]),
        ] {
            assert!(trial_report(bad).is_err());
        }
    }
}

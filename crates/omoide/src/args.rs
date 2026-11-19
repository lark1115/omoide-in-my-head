use clap::{Args, Parser, Subcommand};
use serde::Serialize;
use std::path::PathBuf;

#[derive(Parser)]
#[command(version, about = "Source-linked local memory")]
pub struct Cli {
    #[arg(long, global = true, env = "OMOIDE_DB", default_value = "omoide.db")]
    pub db: PathBuf,
    #[arg(long, global = true)]
    pub operation_id: Option<String>,
    #[arg(long, global = true)]
    pub opportunity_id: Option<String>,
    #[arg(long, global = true)]
    pub call_id: Option<String>,
    #[command(subcommand)]
    pub command: Command,
}
#[derive(Subcommand, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Command {
    Init,
    Say {
        #[arg(long)]
        text: String,
        #[arg(long, default_value = "user")]
        speaker: String,
        /// Original utterance time, in Unix milliseconds. Defaults to registration time.
        #[arg(long)]
        at: Option<i64>,
        #[arg(long)]
        target: Option<String>,
    },
    Rule {
        #[command(subcommand)]
        command: RuleCommand,
    },
    Resolve {
        /// Exact pending version, as returned by rule add.
        pending: String,
        #[arg(long, value_parser=["current","pending"])]
        keep: String,
    },
    Fact {
        #[command(subcommand)]
        command: FactCommand,
    },
    Decide(Decision),
    Event {
        #[command(subcommand)]
        command: EventCommand,
    },
    Used(Notice),
    Acted(Notice),
    Corrected(Notice),
    Outcome {
        #[command(flatten)]
        notice: Notice,
        #[arg(long,value_parser=["success","failure"])]
        result: String,
        #[arg(long, env = "OMOIDE_DEMOTE_AFTER", default_value_t = 0)]
        demote_after: u32,
    },
    Stats {
        #[arg(long,default_value_t=2,value_parser=clap::value_parser!(u32).range(1..))]
        correction_threshold: u32,
    },
    Hot {
        #[command(subcommand)]
        command: HotCommand,
    },
    Skill {
        #[command(subcommand)]
        command: SkillCommand,
    },
    Repeated {
        #[arg(long,default_value_t=2,value_parser=clap::value_parser!(u32).range(2..))]
        min: u32,
    },
    Eval {
        #[command(subcommand)]
        command: EvalCommand,
    },
    Recall(Query),
    Show {
        id: String,
        #[arg(long)]
        history: bool,
    },
}
#[derive(Subcommand, Serialize)]
pub enum RuleCommand {
    Add(Rule),
}
#[derive(Args, Serialize)]
pub struct Rule {
    #[arg(long)]
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub pinned: bool,
    #[arg(long)]
    pub source: String,
    #[arg(long)]
    pub text: String,
    #[arg(long)]
    pub scene: String,
    #[arg(long = "alias")]
    pub aliases: Vec<String>,
    #[arg(long = "action")]
    pub actions: Vec<String>,
    #[arg(long)]
    pub target: Option<String>,
    #[arg(long)]
    pub purpose: Option<String>,
    #[arg(long)]
    pub exclude: Option<String>,
    #[arg(long,value_parser=["same","restated","exception","unrelated","unsure"],requires_all=["against","near_digest"])]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub relation: Option<String>,
    #[arg(long, requires = "relation")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub against: Option<String>,
    #[arg(long, requires = "relation")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub near_digest: Option<String>,
}
#[derive(Args, Serialize)]
pub struct Query {
    #[command(flatten)]
    pub judging: JudgeConfig,
    #[arg(long, required_unless_present = "action")]
    pub scene: Option<String>,
    #[arg(long, required_unless_present = "scene")]
    pub action: Option<String>,
    #[arg(long)]
    pub target: Option<String>,
    #[arg(long)]
    pub cursor: Option<String>,
    #[arg(long,value_parser=["rule","skill","decision","fact","experience"])]
    pub kind: Option<String>,
    #[arg(long)]
    pub period: Option<String>,
    #[arg(long)]
    pub scope: Vec<String>,
    #[arg(long)]
    pub claim: Option<String>,
    #[arg(long, env = "OMOIDE_LOG_QUERY")]
    pub log_query: bool,
    #[arg(long, env = "OMOIDE_SCAN_LIMIT", default_value_t = 500)]
    pub scan_limit: u32,
    #[arg(long, env = "OMOIDE_FTS_LIMIT", default_value_t = 500)]
    pub fts_limit: u32,
}

#[derive(Subcommand, Serialize)]
pub enum FactCommand {
    Add {
        #[arg(long)]
        source: String,
        #[arg(long)]
        text: String,
        #[arg(long)]
        target: Option<String>,
        #[arg(long,default_value="unknown",value_parser=["reported","unknown","verified","self-verified"])]
        certainty: String,
        #[command(flatten)]
        #[serde(flatten)]
        attributes: FactAttributes,
    },
}

#[derive(Args, Serialize)]
pub struct Decision {
    /// Replace this exact decision version, preserving its original words and source.
    #[arg(long)]
    pub revises: Option<String>,
    #[arg(long)]
    pub source: String,
    #[arg(long)]
    pub text: String,
    #[arg(long)]
    pub target: Option<String>,
    #[arg(long)]
    pub at: Option<i64>,
    #[arg(long,value_parser=["instruction","proposal"])]
    pub speech: String,
    #[arg(long,value_parser=["pending","decided","revised"])]
    pub state: String,
    #[arg(long)]
    pub scope: Vec<String>,
    #[arg(long)]
    pub period: Option<String>,
}
#[derive(Args, Serialize)]
pub struct FactAttributes {
    #[arg(long)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub at: Option<i64>,
    #[arg(long)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub location: Option<String>,
    #[arg(long)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub evidence: Option<String>,
    #[arg(long)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub verified_where: Option<String>,
    #[arg(long)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub verified_when: Option<i64>,
    #[arg(long)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub verified_how: Option<String>,
    #[arg(long)]
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub scope: Vec<String>,
    #[arg(long)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub period: Option<String>,
    #[arg(long)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub claim: Option<String>,
}
#[derive(Subcommand, Serialize)]
pub enum EventCommand {
    Add {
        #[arg(long)]
        text: String,
        #[arg(long)]
        at: Option<i64>,
        #[arg(long)]
        target: Option<String>,
        #[arg(long)]
        scope: Vec<String>,
        #[arg(long)]
        period: Option<String>,
    },
}
#[derive(Args, Serialize)]
pub struct Notice {
    #[arg(long)]
    pub version: String,
    #[arg(long)]
    pub notice_id: Option<String>,
    #[arg(long)]
    pub action: Option<String>,
    /// Call that originally returned this version.
    #[arg(long)]
    pub from_call: Option<String>,
}

impl Command {
    pub fn notice(&self) -> Option<(&str, &Notice, Option<&str>)> {
        match self {
            Self::Used(n) => Some(("used", n, None)),
            Self::Acted(n) => Some(("acted", n, None)),
            Self::Corrected(n) => Some(("corrected", n, None)),
            Self::Outcome { notice, result, .. } => Some(("outcome", notice, Some(result))),
            _ => None,
        }
    }
}

#[derive(Args, Serialize)]
pub struct HotFile {
    pub file: PathBuf,
    #[arg(long, env = "OMOIDE_HOT_BYTES", default_value_t = 4096)]
    pub limit_bytes: usize,
}
#[derive(Subcommand, Serialize)]
pub enum HotCommand {
    Init(HotFile),
    Export(HotFile),
    Status(HotFile),
}

#[derive(Subcommand, Serialize)]
pub enum SkillCommand {
    Propose {
        #[arg(long = "source", required = true)]
        sources: Vec<String>,
        #[arg(long)]
        text: String,
        #[arg(long)]
        scene: String,
        #[arg(long = "action")]
        actions: Vec<String>,
        #[arg(long)]
        target: Option<String>,
        #[arg(long)]
        reference: Option<String>,
    },
    Confirm {
        version: String,
        #[arg(long,value_parser=["same","restated","exception","unrelated","unsure"],requires_all=["against","near_digest"])]
        relation: Option<String>,
        #[arg(long, requires = "relation")]
        against: Option<String>,
        #[arg(long, requires = "relation")]
        near_digest: Option<String>,
    },
}
#[derive(Subcommand, Serialize)]
pub enum EvalCommand {
    Run {
        #[arg(long, default_value = "tests/scenes")]
        scenes: PathBuf,
        #[arg(long)]
        binary: Option<PathBuf>,
        #[arg(long)]
        fault_binary: Option<PathBuf>,
        #[arg(long)]
        judge_fixture: Option<PathBuf>,
        #[arg(long)]
        aliases: Option<PathBuf>,
        #[arg(long)]
        oracle: bool,
        #[arg(long)]
        scene: Option<String>,
        #[arg(long)]
        trials: Option<PathBuf>,
    },
    Diff {
        before: PathBuf,
        after: PathBuf,
        /// Compare two executable builds directly; otherwise compare saved reports.
        #[arg(long)]
        builds: bool,
        #[arg(long, default_value = "tests/scenes")]
        scenes: PathBuf,
        #[arg(long)]
        before_fault: Option<PathBuf>,
        #[arg(long)]
        after_fault: Option<PathBuf>,
        #[arg(long)]
        scene: Option<String>,
        #[arg(long)]
        allow: Option<PathBuf>,
    },
}
#[derive(Args, Serialize)]
pub struct JudgeConfig {
    #[arg(long, env = "OMOIDE_JUDGE")]
    pub judge: Option<PathBuf>,
    #[arg(long = "judge-arg", allow_hyphen_values = true)]
    pub args: Vec<String>,
    #[arg(long, env = "OMOIDE_JUDGE_TIMEOUT_MS", default_value_t = 1000)]
    pub timeout_ms: u64,
    #[arg(long, env = "OMOIDE_JUDGE_OUTPUT_BYTES", default_value_t = 262144)]
    pub output_bytes: usize,
    #[arg(long, env = "OMOIDE_JUDGE_THRESHOLD", default_value_t = 0.5)]
    pub threshold: f64,
}

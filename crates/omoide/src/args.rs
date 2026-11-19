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
    #[arg(long, required_unless_present = "action")]
    pub scene: Option<String>,
    #[arg(long, required_unless_present = "scene")]
    pub action: Option<String>,
    #[arg(long)]
    pub target: Option<String>,
    #[arg(long)]
    pub cursor: Option<String>,
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
        #[arg(long,default_value="unknown",value_parser=["reported","unknown"])]
        certainty: String,
    },
}

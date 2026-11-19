mod args;
mod db;
mod search;
mod write;
use args::{Cli, Command};
use clap::Parser;
use omoide_store::{Error, Sqlite};
use serde_json::{Value, json};
use std::process::ExitCode;
use uuid::Uuid;
fn run(cli: &Cli, op: &str) -> db::Result<Value> {
    let init = matches!(cli.command, Command::Init);
    write::nonempty(op)?;
    if op.len() > 4096 {
        return Err(Error::Input("operation id too long".into()));
    }
    let mut db = Sqlite::open(&cli.db, init)?;
    if init {
        let store_id = Uuid::new_v4().to_string();
        let ready = json!({"state":"ready","store_id":store_id,"operation_id":op});
        let hash = db::digest(json!(Command::Init).to_string());
        let receipt = omoide_store::save_receipt(op, &hash, &ready.to_string());
        if db.initialize(&store_id, &omoide_store::guarded(0, vec![receipt]))? {
            return Ok(ready);
        }
    }
    match &cli.command {
        Command::Show { id } => db::show(&mut db, id),
        Command::Recall(q) => search::recall(&mut db, q),
        cmd => write::apply(&mut db, cmd, op),
    }
}
fn main() -> ExitCode {
    let cli = match Cli::try_parse() {
        Ok(c) => c,
        Err(e) => {
            let code = e.exit_code();
            println!(
                "{}",
                json!({"state":if code==0{"help"}else{"error"},"kind":"arguments","message":e.to_string()})
            );
            return ExitCode::from(code as u8);
        }
    };
    let op = cli
        .operation_id
        .clone()
        .unwrap_or_else(|| Uuid::new_v4().to_string());
    match run(&cli, &op) {
        Ok(v) => {
            println!("{v}");
            ExitCode::SUCCESS
        }
        Err(e) => {
            let kind = match e {
                Error::Revision => "revision",
                Error::Busy => "contention",
                Error::Input(_) => "invalid_input",
                Error::Full => "capacity",
                Error::Unknown(_) => "storage_unknown",
            };
            println!(
                "{}",
                json!({"state":"error","kind":kind,"message":e.to_string(),"operation_id":op})
            );
            ExitCode::FAILURE
        }
    }
}

mod activity;
mod args;
mod coverage;
mod db;
mod eval;
mod hot;
mod judge;
mod judging;
mod memory;
mod relations;
mod search;
mod skills;
mod write;
use args::{Cli, Command};
use clap::Parser;
use omoide_store::{Error, Sqlite};
use serde_json::{Value, json};
use std::process::ExitCode;
use uuid::Uuid;
fn run(cli: &Cli, op: &str) -> db::Result<Value> {
    if let Command::Eval { command } = &cli.command {
        return eval::run(command);
    }
    let clock = std::time::Instant::now();
    let at = write::now()?;
    let call_id = cli
        .call_id
        .clone()
        .unwrap_or_else(|| Uuid::new_v4().to_string());
    write::nonempty(&call_id)?;
    if let Some(key) = &cli.opportunity_id {
        write::nonempty(key)?;
    }
    let init = matches!(cli.command, Command::Init);
    write::nonempty(op)?;
    if op.len() > 4096 {
        return Err(Error::Input("operation id too long".into()));
    }
    let mut db = Sqlite::open(&cli.db, init)?;
    let initialized = if init {
        let store_id = Uuid::new_v4().to_string();
        let ready = json!({"state":"ready","store_id":store_id,"operation_id":op});
        let hash = db::digest(json!(Command::Init).to_string());
        let receipt = omoide_store::save_receipt(op, &hash, &ready.to_string());
        if db.initialize(&store_id, &omoide_store::guarded(0, vec![receipt]))? {
            Some(ready)
        } else {
            None
        }
    } else {
        None
    };
    let mut result = if let Some(ready) = initialized {
        ready
    } else {
        match &cli.command {
            Command::Hot { command } => hot::run(&mut db, command, &cli.db)?,
            Command::Repeated { min } => skills::repeated(&mut db, *min)?,
            Command::Show { id, history } => db::show(&mut db, id, *history)?,
            Command::Stats {
                correction_threshold,
            } => activity::stats(&mut db, *correction_threshold)?,
            Command::Recall(query) => search::recall(&mut db, query, &call_id)?,
            cmd => write::apply(&mut db, cmd, op, cli.opportunity_id.as_deref(), &call_id)?,
        }
    };
    if let Err(err) = activity::log(
        &mut db,
        &cli.command,
        &call_id,
        &cli.opportunity_id,
        at,
        clock.elapsed().as_micros() as i64,
        &result,
    ) {
        eprintln!(
            "{}",
            json!({"warning":"call_log_failed","message":err.to_string()})
        );
        if matches!(cli.command, Command::Recall(_)) {
            result["accounting"] = json!("unlogged");
        }
    }
    Ok(result)
}
fn main() -> ExitCode {
    // Private handshake for the opt-in fault runner. Normal CLI stdout remains JSON.
    #[cfg(feature = "fault-injection")]
    if std::env::var_os("OMOIDE_TEST_SIGNAL_START").is_some() {
        use std::io::Write;
        let mut out = std::io::stdout().lock();
        if out
            .write_all(b"OMOIDE_FAULT_READY\n")
            .and_then(|()| out.flush())
            .is_err()
        {
            return ExitCode::FAILURE;
        }
    }
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
    let notice_id = cli
        .command
        .notice()
        .and_then(|(_, n, _)| n.notice_id.clone());
    if let (Some(id), Some(op)) = (&notice_id, &cli.operation_id)
        && id != op
    {
        println!(
            "{}",
            json!({"state":"error","kind":"invalid_input","message":"notice and operation ids disagree"})
        );
        return ExitCode::FAILURE;
    }
    let op = notice_id
        .or_else(|| cli.operation_id.clone())
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

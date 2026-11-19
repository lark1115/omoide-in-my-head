//! Offline port-test driver: replace HTTP with a recorded response, using the production wire codec.
#[path = "../src/protocol.rs"]
mod protocol;
use std::{env, fs, io, process::ExitCode};

fn replay() -> Result<serde_json::Value, &'static str> {
    let input: protocol::Input =
        serde_json::from_reader(io::stdin().lock()).map_err(|_| "invalid port input")?;
    if input.is_empty() {
        return Ok(serde_json::json!({"answers":[]}));
    }
    input.request()?;
    let path = env::args_os().nth(1).ok_or("recording required")?;
    let bytes = fs::read(path).map_err(|_| "unreadable recording")?;
    input.decode(&bytes).map(|(response, _)| response)
}
fn main() -> ExitCode {
    match replay() {
        Ok(v) => {
            println!("{v}");
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("{e}");
            ExitCode::FAILURE
        }
    }
}

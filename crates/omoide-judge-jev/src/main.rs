mod protocol;

use protocol::{Input, Usage};
use serde_json::{Value, json};
use std::{
    env, io,
    process::ExitCode,
    time::{Duration, Instant},
};
use ureq::{
    config::Config,
    http::{HeaderValue, Request, Response},
};

const ENDPOINT: &str = "https://api.typesafe.ai/v1/systemone";
const DEFAULT_TIMEOUT_MS: u64 = 900;

fn remaining(budget: Duration, elapsed: Duration) -> Result<Duration, &'static str> {
    budget
        .checked_sub(elapsed)
        .filter(|d| !d.is_zero())
        .ok_or("adapter deadline exceeded")
}

fn post_with(
    body: &Value,
    key: &str,
    timeout: Duration,
    exchange: impl FnOnce(Config, Request<Vec<u8>>) -> Result<Response<ureq::Body>, ureq::Error>,
) -> Result<Vec<u8>, String> {
    let config = ureq::Agent::config_builder()
        .https_only(true)
        .max_redirects(0)
        .timeout_global(Some(timeout))
        .build();
    let mut bearer = HeaderValue::from_str(&format!("Bearer {key}"))
        .map_err(|_| "invalid authorization header")?;
    bearer.set_sensitive(true);
    let req = Request::post(ENDPOINT)
        .header("Authorization", bearer)
        .header("Content-Type", "application/json")
        .body(body.to_string().into_bytes())
        .map_err(|_| "invalid TypeSafe request")?;
    let mut response = exchange(config, req).map_err(|err| match err {
        ureq::Error::StatusCode(code) => format!("TypeSafe HTTP {code}"),
        ureq::Error::Timeout(_) => "TypeSafe request timed out".into(),
        _ => "TypeSafe transport failed".into(),
    })?;
    if !response.status().is_success() {
        return Err(format!("TypeSafe HTTP {}", response.status().as_u16()));
    }
    response
        .body_mut()
        .read_to_vec()
        .map_err(|_| "could not read TypeSafe response".into())
}

fn evaluate(
    input: &Input,
    key: Option<&str>,
    budget: Duration,
    start: Instant,
    post: impl FnOnce(&Value, &str, Duration) -> Result<Vec<u8>, String>,
) -> Result<(Value, Option<Usage>), String> {
    let key = key
        .filter(|s| !s.trim().is_empty())
        .ok_or("missing TYPESAFE_API_KEY")?;
    if input.is_empty() {
        return Ok((json!({"answers":[]}), None));
    }
    let request = input.request()?;
    let bytes = post(&request, key, remaining(budget, start.elapsed())?)?;
    let (response, tokens) = input.decode(&bytes)?;
    remaining(budget, start.elapsed())?;
    Ok((response, Some(tokens)))
}
fn run() -> Result<Value, String> {
    let start = Instant::now();
    let report = match env::args().skip(1).collect::<Vec<_>>().as_slice() {
        [] => false,
        [arg] if arg == "--report-usage" => true,
        _ => return Err("expected no arguments or --report-usage".into()),
    };
    let ms = match env::var("JEV_TIMEOUT_MS") {
        Ok(s) => s.parse::<u64>().map_err(|_| "invalid JEV_TIMEOUT_MS")?,
        Err(env::VarError::NotPresent) => DEFAULT_TIMEOUT_MS,
        Err(_) => return Err("invalid JEV_TIMEOUT_MS".into()),
    };
    let key = env::var("TYPESAFE_API_KEY").ok();
    let input: Input =
        serde_json::from_reader(io::stdin().lock()).map_err(|_| "invalid judge input")?;
    let (response, usage) = evaluate(
        &input,
        key.as_deref(),
        Duration::from_millis(ms),
        start,
        |body, key, timeout| {
            post_with(body, key, timeout, |config, req| {
                config.new_agent().run(req)
            })
        },
    )?;
    if report && let Some(tokens) = usage {
        eprintln!("{}", json!({"model":protocol::MODEL,"usage":tokens}));
    }
    Ok(response)
}
fn main() -> ExitCode {
    match run() {
        Ok(reply) => {
            println!("{reply}");
            ExitCode::SUCCESS
        }
        Err(reason) => {
            eprintln!("{reason}");
            if matches!(
                reason.as_str(),
                "TypeSafe request timed out" | "adapter deadline exceeded"
            ) {
                ExitCode::from(124)
            } else {
                ExitCode::FAILURE
            }
        }
    }
}
#[cfg(test)]
mod tests;

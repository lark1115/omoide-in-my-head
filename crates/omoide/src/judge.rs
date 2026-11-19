use crate::args::JudgeConfig;
use serde_json::{Value as Json, json};
use std::{
    collections::{BTreeMap, BTreeSet},
    io::{Read, Seek, Write},
    os::unix::{io::AsRawFd, process::CommandExt},
    process::{Command, Stdio},
    thread,
    time::{Duration, Instant},
};

fn nonblocking(fd: std::os::fd::RawFd) -> Result<(), String> {
    // F_SETFL returns zero on success and -1 on error.
    if unsafe { libc::fcntl(fd, libc::F_SETFL, libc::O_NONBLOCK) } == -1 {
        Err("cannot bound judge output".into())
    } else {
        Ok(())
    }
}
fn retry_read(kind: std::io::ErrorKind) -> Result<(), String> {
    match kind {
        std::io::ErrorKind::WouldBlock | std::io::ErrorKind::Interrupted => Ok(()),
        _ => Err("judge output read failed".into()),
    }
}
pub fn ask(
    config: &JudgeConfig,
    query: &Json,
    pool: &BTreeMap<String, (bool, Json)>,
) -> Result<BTreeMap<String, usize>, String> {
    let exe = config.judge.as_ref().ok_or("judge is not configured")?;
    let candidates=pool.values().map(|(_,r)|json!({"version_id":r["version_id"],"text":r["text"],"scene":r["scene"],"aliases":r["aliases"]})).collect::<Vec<_>>();
    let ids: BTreeSet<String> = pool
        .values()
        .map(|(_, r)| r["version_id"].as_str().unwrap().to_owned())
        .collect();
    let mut input = tempfile::tempfile().map_err(|e| e.to_string())?;
    serde_json::to_writer(&mut input, &json!({"query":query,"candidates":candidates}))
        .map_err(|e| e.to_string())?;
    input
        .write_all(b"\n")
        .and_then(|()| input.rewind())
        .map_err(|e| e.to_string())?;
    let mut child = Command::new(exe)
        .args(&config.args)
        .stdin(input)
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .process_group(0)
        .spawn()
        .map_err(|_| "judge launch failed")?;

    let result: Result<(std::process::ExitStatus, Vec<u8>), String> = (|| {
        let mut output = child.stdout.take().unwrap();
        let fd = output.as_raw_fd();
        nonblocking(fd)?;
        let start = Instant::now();
        let mut bytes = Vec::new();
        let mut block = [0u8; 8192];
        loop {
            if start.elapsed() >= Duration::from_millis(config.timeout_ms) {
                return Err("judge timeout".into());
            }
            match output.read(&mut block) {
                Ok(0) => {
                    if let Some(status) = child.try_wait().map_err(|_| "judge wait failed")? {
                        return Ok((status, bytes));
                    }
                }
                Ok(n) => {
                    if n > config.output_bytes.saturating_sub(bytes.len()) {
                        return Err("judge output limit".into());
                    }
                    bytes.extend_from_slice(&block[..n]);
                    continue;
                }
                Err(e) => retry_read(e.kind())?,
            }
            thread::sleep(Duration::from_millis(1));
        }
    })();
    if result.is_err() {
        // SAFETY: we created this unreaped child as leader of its own process group.
        unsafe {
            libc::kill(-(child.id() as i32), libc::SIGKILL);
        }
        let _ = child.wait();
    }
    let (status, bytes) = result?;
    if !status.success() {
        return Err("judge exited unsuccessfully".into());
    }
    omoide_core::judge::scores(&bytes, &ids, config.threshold)
}

#[cfg(test)]
mod tests {
    use super::*;
    // [REQ-JUDGE-1]
    #[test]
    fn only_transient_read_failures_can_be_retried() {
        assert!(nonblocking(-1).is_err());
        assert!(retry_read(std::io::ErrorKind::WouldBlock).is_ok());
        assert!(retry_read(std::io::ErrorKind::Interrupted).is_ok());
        assert!(retry_read(std::io::ErrorKind::BrokenPipe).is_err());
    }
}

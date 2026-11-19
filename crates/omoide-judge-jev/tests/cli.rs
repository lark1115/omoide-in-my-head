use std::{
    io::Write,
    process::{Command, Stdio},
};

fn invoke(args: &[&str], timeout: Option<&str>) -> (bool, String, String) {
    // Deliberately remove the caller's credential before starting any test process.
    let mut command = Command::new(env!("CARGO_BIN_EXE_omoide-judge-jev"));
    command
        .env_remove("TYPESAFE_API_KEY")
        .env_remove("JEV_TIMEOUT_MS")
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    if let Some(value) = timeout {
        command.env("JEV_TIMEOUT_MS", value);
    }
    let mut child = command.spawn().unwrap();
    // An argument error can exit before reading stdin.
    let _ = child
        .stdin
        .take()
        .unwrap()
        .write_all(br#"{"query":{},"candidates":[]}"#);
    let out = child.wait_with_output().unwrap();
    (
        out.status.success(),
        String::from_utf8(out.stdout).unwrap(),
        String::from_utf8(out.stderr).unwrap(),
    )
}

// [REQ-JUDGE-1]
#[test]
fn cli_failures_never_supply_scores_or_require_network() {
    for (args, timeout, message) in [
        (vec![], None, "missing TYPESAFE_API_KEY"),
        (vec!["--report-usage"], None, "missing TYPESAFE_API_KEY"),
        (
            vec!["--report-other"],
            None,
            "expected no arguments or --report-usage",
        ),
        (vec![], Some("invalid"), "invalid JEV_TIMEOUT_MS"),
    ] {
        let (ok, stdout, stderr) = invoke(&args, timeout);
        assert!(!ok);
        assert!(stdout.is_empty());
        assert_eq!(stderr.trim(), message);
    }
}

// [REQ-JUDGE-1]
#[test]
fn recognizes_only_the_documented_key_and_timeout_names_offline() {
    let spawn = |extra: &[(&str, &str)]| {
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_omoide-judge-jev"));
        cmd.env_clear()
            .envs(extra.iter().copied())
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        let mut p = cmd.spawn().unwrap();
        // Even a mutation accepting duplicate IDs cannot reach the network with a zero deadline.
        let _=p.stdin.take().unwrap().write_all(br#"{"query":{},"candidates":[{"version_id":"v","text":"x"},{"version_id":"v","text":"y"}]}"#);
        let output = p.wait_with_output().unwrap();
        assert!(!output.status.success());
        assert!(output.stdout.is_empty());
        String::from_utf8(output.stderr).unwrap()
    };
    assert_eq!(
        spawn(&[
            ("JEV_TIMEOUT_MS", "0"),
            ("TYPESAFE_API_KEY", "ephemeral-marker")
        ])
        .trim(),
        "invalid candidate IDs"
    );
    assert_eq!(
        spawn(&[
            ("JEV_TIMEOUT_MS", "0"),
            ("OTHER_API_KEY", "ephemeral-marker")
        ])
        .trim(),
        "missing TYPESAFE_API_KEY"
    );
}

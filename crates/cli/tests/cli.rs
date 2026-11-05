//! The exit codes are the CLI's contract with CI jobs and update scripts.

use std::process::{Command, Output};

fn run(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_jarsnitch")).args(args).output().expect("run jarsnitch")
}

fn jar(name: &str) -> String {
    format!("{}/../../fixtures/jars/{name}.jar", env!("CARGO_MANIFEST_DIR"))
}

fn code(args: &[&str]) -> i32 {
    run(args).status.code().expect("exit code")
}

#[test]
fn scan_exit_codes() {
    assert_eq!(code(&["scan", &jar("benign-plugin")]), 0);
    assert_eq!(code(&["scan", &jar("corr-download-and-run")]), 2);
    assert_eq!(code(&["scan", "does-not-exist.jar"]), 1);
    assert_eq!(code(&["scan", &jar("benign-plugin"), "does-not-exist.jar"]), 1);
    assert_eq!(code(&["scan", &jar("corr-download-and-run"), "does-not-exist.jar"]), 2, "High wins");
}

#[test]
fn scanning_nothing_is_a_failure() {
    let empty = std::env::temp_dir().join(format!("jarsnitch-empty-{}", std::process::id()));
    std::fs::create_dir_all(&empty).unwrap();
    let got = code(&["scan", empty.to_str().unwrap()]);
    std::fs::remove_dir(&empty).unwrap();
    assert_eq!(got, 1);
}

#[test]
fn diff_exit_codes() {
    assert_eq!(code(&["diff", &jar("diff-v1"), &jar("diff-v2")]), 0);
    assert_eq!(code(&["diff", &jar("benign-plugin"), &jar("corr-download-and-run")]), 2);
    assert_eq!(code(&["diff", &jar("corr-download-and-run"), &jar("benign-plugin")]), 0, "removing High is fine");
    assert_eq!(code(&["diff", &jar("diff-v1"), "does-not-exist.jar"]), 1);
    assert_eq!(code(&["diff", &jar("diff-v1")]), 1, "needs two jars");
}

#[test]
fn flags() {
    let version = run(&["--version"]);
    assert!(version.status.success());
    assert_eq!(String::from_utf8_lossy(&version.stdout).trim(), concat!("jarsnitch ", env!("CARGO_PKG_VERSION")));
    assert!(run(&["scan", "--help"]).status.success());

    let unknown = run(&["scan", &jar("benign-plugin"), "--jsn"]);
    assert_eq!(unknown.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&unknown.stderr).contains("unknown option --jsn"));
    assert!(unknown.stdout.is_empty(), "doesn't scan anything");
}

#[test]
fn json_is_one_array_per_scan_and_one_object_per_diff() {
    let scan = run(&["scan", &jar("benign-plugin"), &jar("net-http"), "--json"]);
    let reports: serde_json::Value = serde_json::from_slice(&scan.stdout).unwrap();
    assert_eq!(reports.as_array().map(Vec::len), Some(2));

    let diff = run(&["diff", &jar("diff-v1"), &jar("diff-v2"), "--json"]);
    let diff: serde_json::Value = serde_json::from_slice(&diff.stdout).unwrap();
    assert!(diff.is_object());
}

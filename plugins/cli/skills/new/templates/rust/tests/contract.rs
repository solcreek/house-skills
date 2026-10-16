// The rubric's must rules, as tests. Run with `cargo test`.
use std::path::Path;

use assert_cmd::Command;
use predicates::prelude::*;
use serde_json::Value;

// stdin at EOF and a timeout, so a command that prompts fails instead of
// hanging; TOOL_HOME isolates any state (conventions).
fn cli(cwd: &Path) -> Command {
    let mut c = Command::cargo_bin("tool").unwrap();
    c.current_dir(cwd)
        .env("NO_COLOR", "1")
        .env("TOOL_HOME", cwd.join(".home"))
        .write_stdin("")
        .timeout(std::time::Duration::from_secs(10));
    c
}

fn sandbox() -> tempfile::TempDir {
    tempfile::tempdir().unwrap()
}

fn json(out: &[u8]) -> Value {
    serde_json::from_slice(out).unwrap()
}

#[test]
fn help_goes_to_stdout_and_exits_0() {
    let d = sandbox();
    for args in [&["--help"][..], &["-h"], &["greet", "--help"], &["touch", "-h"], &["touch", "x", "--help"]] {
        cli(d.path()).args(args).assert().success().stdout(predicate::str::contains("Usage")).stderr("");
    }
    assert!(!d.path().join("x").exists());
}

#[test]
fn usage_errors_exit_2() {
    let d = sandbox();
    for args in [&["--zz"][..], &["nope"], &["greet"], &["greet", "Ada", "Bob"], &["greet", "Ada", "--zz"], &["describe", "zz"]] {
        cli(d.path()).args(args).assert().code(2).stdout("").stderr(predicate::str::contains("--help"));
    }
}

#[test]
fn json_is_one_document_failures_included() {
    let d = sandbox();
    let out = cli(d.path()).args(["greet", "Ada", "--json"]).output().unwrap();
    assert_eq!(json(&out.stdout)["greeting"], "hello, Ada");
    for args in [&["greet", "--json"][..], &["greet", "--zz", "--json"]] {
        let out = cli(d.path()).args(args).output().unwrap();
        assert_eq!(out.status.code(), Some(2));
        assert_eq!(json(&out.stdout)["usage"], true);
    }
}

#[test]
fn mutating_needs_exactly_one_of_dry_run_and_yes() {
    let d = sandbox();
    for flags in [&[][..], &["--dry-run", "--yes"]] {
        cli(d.path()).args(["touch", "x"]).args(flags).assert().code(2).stderr(predicate::str::contains("--dry-run"));
    }
    assert!(!d.path().join("x").exists());
}

#[test]
fn dry_run_changes_nothing_yes_applies_rerun_succeeds() {
    let d = sandbox();
    let dry = json(&cli(d.path()).args(["touch", "x", "--dry-run", "--json"]).output().unwrap().stdout);
    assert_eq!((dry["changed"].as_bool(), dry["dryRun"].as_bool()), (Some(true), Some(true)));
    assert!(!d.path().join("x").exists());
    let yes = json(&cli(d.path()).args(["touch", "x", "--yes", "--json"]).output().unwrap().stdout);
    assert_eq!(yes["changed"], true);
    assert!(d.path().join("x").exists());
    let again = cli(d.path()).args(["touch", "x", "--yes", "--json"]).output().unwrap();
    assert!(again.status.success());
    assert_eq!(json(&again.stdout)["changed"], false);
}

#[test]
fn output_path_cannot_escape_the_current_directory() {
    let d = sandbox();
    std::os::unix::fs::symlink(std::env::temp_dir(), d.path().join("out")).unwrap();
    for path in ["../x", "/tmp/x", "out/x"] {
        let out = cli(d.path()).args(["touch", path, "--yes", "--json"]).output().unwrap();
        assert_eq!(out.status.code(), Some(1), "{path}");
        assert!(json(&out.stdout)["error"].as_str().unwrap().contains("outside"), "{path}");
    }
}

#[test]
fn describe_lists_every_command_with_its_flags() {
    let d = sandbox();
    let v = json(&cli(d.path()).arg("describe").output().unwrap().stdout);
    assert_eq!(v["exitCodes"]["usage"], 2);
    let names = |c: &Value| -> Vec<String> {
        let mut n: Vec<String> = c["flags"].as_array().unwrap().iter().map(|f| f["name"].as_str().unwrap().to_string()).collect();
        n.sort();
        n
    };
    assert!(names(&v).contains(&"version".to_string()));
    let cmd = |name: &str| v["commands"].as_array().unwrap().iter().find(|c| c["name"] == name).unwrap().clone();
    assert_eq!(names(&cmd("greet")), ["help", "json"]);
    assert_eq!(names(&cmd("touch")), ["dry-run", "help", "json", "yes"]);
    assert_eq!(cmd("touch")["mutates"], true);
}

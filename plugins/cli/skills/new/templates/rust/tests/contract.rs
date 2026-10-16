// The rubric's must rules, as tests. Run with `cargo test`.
use std::path::Path;

use assert_cmd::Command;
use predicates::prelude::*;
use serde_json::Value;

// stdin at EOF and a timeout, so a command that prompts fails instead of
// hanging; TOOL_HOME isolates state (conventions).
fn cli_in(cwd: &Path, home: &Path) -> Command {
    let mut c = Command::cargo_bin("tool").unwrap();
    c.current_dir(cwd)
        .env("NO_COLOR", "1")
        .env("TOOL_HOME", home)
        .write_stdin("")
        .timeout(std::time::Duration::from_secs(10));
    c
}

fn cli(cwd: &Path) -> Command {
    cli_in(cwd, &cwd.join(".home"))
}

fn sandbox() -> tempfile::TempDir {
    tempfile::tempdir().unwrap()
}

fn json(out: &[u8]) -> Value {
    serde_json::from_slice(out).unwrap()
}

// A directory with three files of different sizes.
fn tree() -> tempfile::TempDir {
    let d = sandbox();
    std::fs::create_dir(d.path().join("sub")).unwrap();
    std::fs::write(d.path().join("a"), "x".repeat(30)).unwrap();
    std::fs::write(d.path().join("b"), "x".repeat(20)).unwrap();
    std::fs::write(d.path().join("sub/c"), "x".repeat(10)).unwrap();
    d
}

#[test]
fn help_goes_to_stdout_and_exits_0() {
    let d = sandbox();
    let cases: [&[&str]; 8] = [
        &[], &["--help"], &["-h"], &["help", "touch"], &["files", "--help"], &["touch", "-h"], &["touch", "x", "--help"], &["describe", "-h"],
    ];
    for args in cases {
        cli(d.path()).args(args).assert().success().stdout(predicate::str::contains("Usage")).stderr("");
    }
    assert!(!d.path().join("x").exists());
    // Exit codes on every help page (rubric E4).
    cli(d.path()).args(["files", "-h"]).assert().stdout(predicate::str::contains("3 partial"));
}

#[test]
fn usage_errors_exit_2() {
    let d = sandbox();
    let cases: [&[&str]; 13] = [
        &["--zz"], &["nope"], &["help", "zz"], &["describe", "zz"],
        &["files", "a", "b"], &["files", "--zz"], &["files", "--limit", "x"], &["files", "--limit=-1"], &["files", "A\u{7}"],
        // A mutating command gets --dry-run, or the missing-flag check would answer first.
        &["touch", "--dry-run"], &["touch", "a", "b", "--dry-run"], &["touch", "", "--dry-run"], &["touch", "x", "--dry-run", "--zz"],
    ];
    for args in cases {
        cli(d.path()).args(args).assert().code(2).stdout("").stderr(predicate::str::contains("--help"));
    }
}

#[test]
fn json_is_one_document_before_or_after_the_command_failures_included() {
    let d = tree();
    let after = cli(d.path()).args(["files", "--json"]).output().unwrap();
    let before = cli(d.path()).args(["--json", "files"]).output().unwrap();
    assert_eq!(json(&after.stdout), json(&before.stdout));
    for args in [&["files", "a", "b", "--json"][..], &["files", "--zz", "--json"], &["touch", "x", "--json"]] {
        let out = cli(d.path()).args(args).output().unwrap();
        assert_eq!(out.status.code(), Some(2));
        assert_eq!(json(&out.stdout)["usage"], true);
    }
    // The usage message under --json names what is missing.
    let out = cli(d.path()).args(["touch", "x", "--json"]).output().unwrap();
    assert!(json(&out.stdout)["error"].as_str().unwrap().contains("--dry-run"));
}

#[test]
fn a_list_is_capped_says_so_on_stderr_and_has_the_list_shape() {
    let d = tree();
    let out = cli(d.path()).args(["files", "--limit", "2", "--json"]).output().unwrap();
    let v = json(&out.stdout);
    assert_eq!((v["total"].as_u64(), v["shown"].as_u64(), v["truncated"].as_bool()), (Some(3), Some(2), Some(true)));
    let paths: Vec<&str> = v["items"].as_array().unwrap().iter().map(|i| i["path"].as_str().unwrap()).collect();
    assert_eq!(paths, ["a", "b"]);
    assert!(String::from_utf8_lossy(&out.stderr).contains("2 of 3 shown; --limit 0 for all"));
    let all = cli(d.path()).args(["files", "--limit", "0", "--json"]).output().unwrap();
    assert_eq!((json(&all.stdout)["truncated"].as_bool(), all.stderr.is_empty()), (Some(false), true));
    cli(d.path()).args(["files", "--limit", "1"]).assert().stdout("30\ta\n");
}

#[test]
fn a_missing_input_fails_naming_it_and_an_unreadable_part_is_partial() {
    use std::os::unix::fs::PermissionsExt;
    let d = tree();
    let out = cli(d.path()).args(["files", "nowhere", "--json"]).output().unwrap();
    assert_eq!(out.status.code(), Some(1));
    assert!(json(&out.stdout)["error"].as_str().unwrap().contains("nowhere"));
    if is_root() {
        return; // root reads everything
    }
    let sub = d.path().join("sub");
    std::fs::set_permissions(&sub, std::fs::Permissions::from_mode(0o000)).unwrap();
    let out = cli(d.path()).args(["files", "--json"]).output().unwrap();
    std::fs::set_permissions(&sub, std::fs::Permissions::from_mode(0o700)).unwrap();
    assert_eq!(out.status.code(), Some(3));
    let v = json(&out.stdout);
    assert_eq!(v["problems"][0]["input"], "sub");
    assert_eq!(v["total"], 2); // the rest is still there
}

// Root reads and writes everything, so permission tests mean nothing there.
fn is_root() -> bool {
    std::process::Command::new("id").arg("-u").output().is_ok_and(|o| o.stdout.starts_with(b"0\n"))
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
fn dry_run_changes_nothing_yes_applies_and_records_state_rerun_succeeds() {
    let d = sandbox();
    let home = sandbox();
    let dry = json(&cli_in(d.path(), home.path()).args(["touch", "x", "--dry-run", "--json"]).output().unwrap().stdout);
    assert_eq!((dry["changed"].as_bool(), dry["dryRun"].as_bool()), (Some(true), Some(true)));
    assert!(!d.path().join("x").exists() && !home.path().join("state").exists());
    let yes = json(&cli_in(d.path(), home.path()).args(["touch", "x", "--yes", "--json"]).output().unwrap().stdout);
    assert_eq!(yes["changed"], true);
    assert!(d.path().join("x").exists());
    let history: Value = json(&std::fs::read(home.path().join("state/history.json")).unwrap());
    assert_eq!(history, serde_json::json!([yes["path"]]));
    let again = cli_in(d.path(), home.path()).args(["touch", "x", "--yes", "--json"]).output().unwrap();
    assert!(again.status.success());
    assert_eq!(json(&again.stdout)["changed"], false);
}

#[test]
fn output_path_cannot_escape_the_current_directory_or_follow_a_symlink() {
    use std::os::unix::fs::symlink;
    let outside = sandbox();
    std::fs::write(outside.path().join("keep"), "precious").unwrap();
    let d = sandbox();
    let cwd = d.path().canonicalize().unwrap();
    std::fs::create_dir(cwd.join("sub")).unwrap();
    symlink(outside.path(), cwd.join("out")).unwrap();
    symlink(outside.path().join("keep"), cwd.join("link")).unwrap();
    symlink(outside.path().join("gone"), cwd.join("dangling")).unwrap();
    symlink(outside.path(), cwd.join("sub/up")).unwrap();
    let abs_sub = format!("{}/sub/", cwd.display());
    let root = cwd.display().to_string();
    let paths = ["../x", "/tmp/x", "out/x", "link", "dangling", "sub/up/../x", "sub/", &abs_sub, ".", &root, "newdir/"];
    for path in paths {
        for mode in ["--dry-run", "--yes"] {
            let out = cli(&cwd).args(["touch", path, mode, "--json"]).output().unwrap();
            assert_eq!(out.status.code(), Some(1), "{path} {mode}");
            assert_eq!(json(&out.stdout)["usage"], false, "{path}");
        }
    }
    assert_eq!(std::fs::read_to_string(outside.path().join("keep")).unwrap(), "precious");
    assert!(!outside.path().join("gone").exists() && !outside.path().join("x").exists());
    assert!(!cwd.join("sub/sub").exists() && !cwd.join("newdir").exists());
}

#[test]
fn absolute_path_inside_is_fine() {
    let d = sandbox();
    let target = d.path().canonicalize().unwrap().join("x");
    cli(d.path()).args(["touch", target.to_str().unwrap(), "--yes"]).assert().success();
}

#[test]
fn a_failed_write_names_the_file() {
    use std::os::unix::fs::PermissionsExt;
    if is_root() {
        return; // root writes anywhere
    }
    let d = sandbox();
    let ro = d.path().join("ro");
    std::fs::create_dir(&ro).unwrap();
    std::fs::set_permissions(&ro, std::fs::Permissions::from_mode(0o500)).unwrap();
    let out = cli(d.path()).args(["touch", "ro/x", "--yes", "--json"]).output().unwrap();
    std::fs::set_permissions(&ro, std::fs::Permissions::from_mode(0o700)).unwrap();
    assert_eq!(out.status.code(), Some(1));
    assert!(json(&out.stdout)["error"].as_str().unwrap().contains("cannot create"));
}

#[test]
fn describe_lists_every_command_its_args_and_flags() {
    let d = sandbox();
    let v = json(&cli(d.path()).arg("describe").output().unwrap().stdout);
    assert_eq!(v["exitCodes"], serde_json::json!({ "ok": 0, "failed": 1, "usage": 2, "partial": 3 }));
    let names = |c: &Value| -> Vec<String> {
        let mut n: Vec<String> = c["flags"].as_array().unwrap().iter().map(|f| f["name"].as_str().unwrap().to_string()).collect();
        n.sort();
        n
    };
    assert_eq!(names(&v), ["help", "json", "version"]);
    let mut keys: Vec<&String> = v.as_object().unwrap().keys().collect();
    keys.sort();
    assert_eq!(keys, ["commands", "exitCodes", "flags", "name", "summary", "version"]);
    let cmd = |name: &str| v["commands"].as_array().unwrap().iter().find(|c| c["name"] == name).unwrap().clone();
    assert_eq!(names(&cmd("files")), ["help", "json", "limit"]);
    assert_eq!(cmd("files")["args"], serde_json::json!([{ "name": "DIR", "required": false, "default": "." }]));
    let limit = cmd("files")["flags"].as_array().unwrap().iter().find(|f| f["name"] == "limit").unwrap().clone();
    assert_eq!((limit["type"].as_str(), limit["default"].as_str()), (Some("integer"), Some("20")));
    assert_eq!(names(&cmd("touch")), ["dry-run", "help", "json", "yes"]);
    assert_eq!(cmd("touch")["mutates"], true);
    assert!(!cmd("files")["examples"].as_array().unwrap().is_empty());
}

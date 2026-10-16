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
    let cases: [&[&str]; 16] = [
        &["--zz"], &["nope"], &["help", "zz"], &["describe", "zz"],
        &["files", "a", "b"], &["files", "--zz"], &["files", "--limit", "x"], &["files", "--limit=-1"], &["files", "A\u{7}"],
        &["files", "--limit", "+2"], &["files", "--limit", "0x3"], &["files", "--limit", "1_0"],
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
    assert!(json(&out.stdout)["error"].as_str().unwrap().contains("no such directory"));
    let out = cli(d.path()).args(["files", "a", "--json"]).output().unwrap();
    assert!(json(&out.stdout)["error"].as_str().unwrap().contains("not a directory"));
    if is_root() {
        return; // root reads everything
    }
    let sub = d.path().join("sub");
    std::fs::set_permissions(&sub, std::fs::Permissions::from_mode(0o000)).unwrap();
    let out = cli(d.path()).args(["files", "--json"]).output().unwrap();
    std::fs::set_permissions(&sub, std::fs::Permissions::from_mode(0o700)).unwrap();
    assert_eq!(out.status.code(), Some(3));
    let v = json(&out.stdout);
    assert_eq!(v["problems"], serde_json::json!([{ "input": "sub", "error": "permission denied" }]));
    assert_eq!(v["total"], 2); // the rest is still there
    // Nothing readable at all is a failure, not a partial result.
    let root = d.path().to_path_buf();
    std::fs::set_permissions(&root, std::fs::Permissions::from_mode(0o000)).unwrap();
    let elsewhere = sandbox();
    let out = cli(elsewhere.path()).args(["files", root.to_str().unwrap(), "--json"]).output().unwrap();
    std::fs::set_permissions(&root, std::fs::Permissions::from_mode(0o700)).unwrap();
    assert_eq!(out.status.code(), Some(1));
    assert!(json(&out.stdout)["error"].as_str().unwrap().contains("permission denied"));
}

#[test]
fn ties_sort_by_bytes_a_symlinked_dir_lists_and_the_home_is_skipped() {
    let d = sandbox();
    for n in ["b2", "B1", "a", "é", "Z", "😀", "Ａ"] {
        std::fs::write(d.path().join(n), "x").unwrap();
    }
    let v = json(&cli(d.path()).args(["files", "--json"]).output().unwrap().stdout);
    let paths: Vec<&str> = v["items"].as_array().unwrap().iter().map(|i| i["path"].as_str().unwrap()).collect();
    assert_eq!(paths, ["B1", "Z", "a", "b2", "é", "Ａ", "😀"]);
    let parent = sandbox();
    std::os::unix::fs::symlink(d.path(), parent.path().join("link")).unwrap();
    let v = json(&cli(parent.path()).args(["files", "link", "--json"]).output().unwrap().stdout);
    assert_eq!(v["total"], 7);
    // A relative TOOL_HOME, with a file directly under it, is skipped whole.
    let home = Path::new(".home");
    cli_in(d.path(), home).args(["touch", "made", "--yes"]).assert().success();
    assert!(d.path().join(".home/state/history.jsonl").exists());
    std::fs::write(d.path().join(".home/extra"), "x").unwrap();
    let v = json(&cli_in(d.path(), home).args(["files", "--json"]).output().unwrap().stdout);
    assert!(v["items"].as_array().unwrap().iter().all(|i| !i["path"].as_str().unwrap().starts_with(".home")));
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
    assert_eq!((yes["changed"].as_bool(), yes["recorded"].as_bool()), (Some(true), Some(true)));
    assert!(d.path().join("x").exists());
    let history = std::fs::read_to_string(home.path().join("state/history.jsonl")).unwrap();
    assert_eq!(history, format!("{}\n", serde_json::json!({ "path": yes["path"] })));
    let again = cli_in(d.path(), home.path()).args(["touch", "x", "--yes", "--json"]).output().unwrap();
    assert!(again.status.success());
    assert_eq!((json(&again.stdout)["changed"].as_bool(), json(&again.stdout)["recorded"].as_bool()), (Some(false), Some(false)));
}

#[test]
fn state_survives_concurrent_runs_and_corruption_and_a_failed_record_warns() {
    use std::os::unix::fs::PermissionsExt;
    let d = sandbox();
    let home = sandbox();
    let state = home.path().join("state");
    std::fs::create_dir(&state).unwrap();
    std::fs::write(state.join("history.jsonl"), "not json\n").unwrap();
    let runs: Vec<_> = (0..12)
        .map(|i| {
            let (cwd, home) = (d.path().to_path_buf(), home.path().to_path_buf());
            std::thread::spawn(move || cli_in(&cwd, &home).args(["touch", &format!("f{i}"), "--yes"]).output().unwrap())
        })
        .collect();
    for r in runs {
        assert!(r.join().unwrap().status.success());
    }
    let lines: Vec<String> = std::fs::read_to_string(state.join("history.jsonl")).unwrap().lines().map(String::from).collect();
    assert_eq!(lines[0], "not json"); // never rewritten
    assert_eq!(lines.len(), 13); // no run lost another's line
    if is_root() {
        return;
    }
    let file = state.join("history.jsonl");
    std::fs::set_permissions(&file, std::fs::Permissions::from_mode(0o400)).unwrap();
    let out = cli_in(d.path(), home.path()).args(["touch", "late", "--yes", "--json"]).output().unwrap();
    std::fs::set_permissions(&file, std::fs::Permissions::from_mode(0o600)).unwrap();
    assert!(out.status.success());
    let v = json(&out.stdout);
    assert_eq!((v["changed"].as_bool(), v["recorded"].as_bool()), (Some(true), Some(false)));
    assert!(String::from_utf8_lossy(&out.stderr).contains("warning: not recorded"));
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
fn forget_rewrites_state_under_its_lock_keeps_unparsed_lines_and_the_mode() {
    use std::os::unix::fs::PermissionsExt;
    let d = sandbox();
    let home = sandbox();
    for n in ["a", "b"] {
        cli_in(d.path(), home.path()).args(["touch", n, "--yes"]).assert().success();
    }
    let file = home.path().join("state/history.jsonl");
    let mut text = std::fs::read_to_string(&file).unwrap();
    text.push_str("not json\n");
    std::fs::write(&file, &text).unwrap();
    std::fs::set_permissions(&file, std::fs::Permissions::from_mode(0o640)).unwrap();
    let dry = json(&cli_in(d.path(), home.path()).args(["forget", "a", "--dry-run", "--json"]).output().unwrap().stdout);
    assert_eq!((dry["removed"].as_u64(), dry["dryRun"].as_bool()), (Some(1), Some(true)));
    assert_eq!(std::fs::read_to_string(&file).unwrap(), text);
    let yes = json(&cli_in(d.path(), home.path()).args(["forget", "a", "--yes", "--json"]).output().unwrap().stdout);
    assert_eq!((yes["removed"].as_u64(), yes["changed"].as_bool()), (Some(1), Some(true)));
    let lines: Vec<String> = std::fs::read_to_string(&file).unwrap().lines().map(String::from).collect();
    assert_eq!(lines.len(), 2);
    assert!(lines[0].contains("/b\"") && lines[1] == "not json");
    assert_eq!(std::fs::metadata(&file).unwrap().permissions().mode() & 0o777, 0o640);
    let again = json(&cli_in(d.path(), home.path()).args(["forget", "a", "--yes", "--json"]).output().unwrap().stdout);
    assert_eq!(again["changed"], false);
}

#[test]
fn a_held_lock_is_busy_and_a_left_lock_file_is_not() {
    let d = sandbox();
    let home = sandbox();
    cli_in(d.path(), home.path()).args(["touch", "a", "--yes"]).assert().success();
    let file = home.path().join("state/history.jsonl");
    let before = std::fs::read_to_string(&file).unwrap();
    // Another run holding the OS lock: this test process.
    let held = std::fs::OpenOptions::new().write(true).open(home.path().join("state/history.jsonl.lock")).unwrap();
    held.lock().unwrap();
    let busy = cli_in(d.path(), home.path()).args(["forget", "a", "--yes", "--json"]).output().unwrap();
    held.unlock().unwrap();
    assert_eq!(busy.status.code(), Some(4));
    assert!(json(&busy.stdout)["error"].as_str().unwrap().contains("locked by another run"));
    assert_eq!(std::fs::read_to_string(&file).unwrap(), before);
    // The file left behind, unlocked (as after a crash): nothing to wait for.
    let taken = cli_in(d.path(), home.path()).args(["forget", "a", "--yes", "--json"]).output().unwrap();
    assert!(taken.status.success());
    assert_eq!(json(&taken.stdout)["removed"], 1);
}

#[test]
fn concurrent_forget_and_touch_lose_nothing_and_revive_nothing() {
    let d = sandbox();
    let home = sandbox();
    for i in 0..12 {
        cli_in(d.path(), home.path()).args(["touch", &format!("p{i}"), "--yes"]).assert().success();
    }
    let runs: Vec<_> = (0..12)
        .flat_map(|i| [vec!["forget".to_string(), format!("p{i}")], vec!["touch".to_string(), format!("n{i}")]])
        .map(|args| {
            let (cwd, home) = (d.path().to_path_buf(), home.path().to_path_buf());
            std::thread::spawn(move || cli_in(&cwd, &home).args(&args).arg("--yes").output().unwrap())
        })
        .collect();
    for r in runs {
        let out = r.join().unwrap();
        assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    }
    let text = std::fs::read_to_string(home.path().join("state/history.jsonl")).unwrap();
    let names: Vec<String> = text
        .lines()
        .map(|l| json(l.as_bytes())["path"].as_str().unwrap().rsplit('/').next().unwrap().to_string())
        .collect();
    let mut want: Vec<String> = (0..12).map(|i| format!("n{i}")).collect();
    let mut got = names.clone();
    want.sort();
    got.sort();
    assert_eq!(got, want, "history: {text}");
}

#[test]
fn forget_finds_a_path_however_it_is_spelled() {
    let d = sandbox();
    let home = sandbox();
    std::fs::create_dir(d.path().join("real")).unwrap();
    std::os::unix::fs::symlink(d.path().join("real"), d.path().join("link")).unwrap();
    cli_in(d.path(), home.path()).args(["touch", "link/n", "--yes"]).assert().success();
    let v = json(&cli_in(d.path(), home.path()).args(["forget", "link/n", "--yes", "--json"]).output().unwrap().stdout);
    assert_eq!(v["removed"], 1);
}

#[test]
fn describe_lists_every_command_its_args_and_flags() {
    let d = sandbox();
    let v = json(&cli(d.path()).arg("describe").output().unwrap().stdout);
    assert_eq!(v["exitCodes"], serde_json::json!({ "ok": 0, "failed": 1, "usage": 2, "partial": 3, "busy": 4 }));
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
    assert_eq!(cmd("forget")["mutates"], true);
    assert!(!cmd("files")["examples"].as_array().unwrap().is_empty());
}

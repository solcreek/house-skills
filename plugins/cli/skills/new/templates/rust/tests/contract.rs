// The rubric's must rules, as tests. Run with `cargo test`.
use assert_cmd::Command;
use predicates::prelude::*;

fn cli() -> Command {
    Command::cargo_bin("tool").unwrap()
}

#[test]
fn help_goes_to_stdout_and_exits_0() {
    for args in [&["--help"][..], &["-h"], &["greet", "--help"]] {
        cli().args(args).assert().success().stdout(predicate::str::contains("Usage")).stderr("");
    }
}

#[test]
fn unknown_flag_is_a_usage_error() {
    cli().args(["greet", "Ada", "--zz"]).assert().code(2).stdout("").stderr(predicate::str::contains("--zz"));
}

#[test]
fn json_is_one_document() {
    let out = cli().args(["greet", "Ada", "--json"]).output().unwrap();
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(v["greeting"], "hello, Ada");
}

#[test]
fn describe_lists_every_command() {
    let out = cli().args(["describe"]).output().unwrap();
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(v["exitCodes"]["usage"], 2);
    let names = |c: &serde_json::Value| -> Vec<String> {
        c["flags"].as_array().unwrap().iter().map(|f| f["name"].as_str().unwrap().to_string()).collect()
    };
    assert!(names(&v).contains(&"version".to_string()));
    let greet = v["commands"].as_array().unwrap().iter().find(|c| c["name"] == "greet").unwrap();
    assert!(names(greet).contains(&"json".to_string()) && names(greet).contains(&"help".to_string()));
}

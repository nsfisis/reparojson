use std::path::PathBuf;
use std::process::{Command, Output};

fn temp_file(name: &str, content: &str) -> PathBuf {
    let path = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(name);
    std::fs::write(&path, content).expect("failed to write file");
    path
}

fn run(args: &[&str], file: Option<&PathBuf>) -> Output {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_reparojson"));
    cmd.args(args);
    if let Some(file) = file {
        cmd.arg(file);
    }
    cmd.output().expect("failed to run reparojson")
}

#[test]
fn repair_file_in_place_rewrites_repaired_file() {
    let path = temp_file("lib_repaired.json", "[ 1 2 ]\n");

    let result = reparojson::repair_file_in_place(path.as_os_str());

    assert!(matches!(result, Ok(reparojson::RepairOk::Repaired)));
    assert_eq!(std::fs::read_to_string(&path).unwrap(), "[ 1, 2 ]\n");
}

#[test]
fn repair_file_in_place_keeps_valid_file() {
    let path = temp_file("lib_valid.json", "[ 1, 2 ]\n");

    let result = reparojson::repair_file_in_place(path.as_os_str());

    assert!(matches!(result, Ok(reparojson::RepairOk::Valid)));
    assert_eq!(std::fs::read_to_string(&path).unwrap(), "[ 1, 2 ]\n");
}

#[test]
fn repair_file_in_place_keeps_invalid_file() {
    let path = temp_file("lib_invalid.json", "[ 1 2 x\n");

    let result = reparojson::repair_file_in_place(path.as_os_str());

    assert!(matches!(result, Err(reparojson::RepairErr::Invalid(_))));
    assert_eq!(std::fs::read_to_string(&path).unwrap(), "[ 1 2 x\n");
}

#[test]
fn repair_file_in_place_fails_if_file_does_not_exist() {
    let path = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("lib_nonexistent.json");

    let result = reparojson::repair_file_in_place(path.as_os_str());

    assert!(matches!(result, Err(reparojson::RepairErr::IoErr(_))));
    assert!(!path.exists());
}

#[test]
fn in_place_flag_rewrites_file_and_prints_nothing() {
    let path = temp_file("cli_repaired.json", "[ 1 2 ]\n");

    let output = run(&["-i"], Some(&path));

    assert_eq!(output.status.code(), Some(0));
    assert!(output.stdout.is_empty());
    assert_eq!(std::fs::read_to_string(&path).unwrap(), "[ 1, 2 ]\n");
}

#[test]
fn in_place_flag_with_strict_rewrites_file_and_fails() {
    let path = temp_file("cli_strict.json", "[ 1 2 ]\n");

    let output = run(&["--in-place", "--strict"], Some(&path));

    assert_eq!(output.status.code(), Some(1));
    assert!(output.stdout.is_empty());
    assert_eq!(std::fs::read_to_string(&path).unwrap(), "[ 1, 2 ]\n");
}

#[test]
fn in_place_flag_keeps_invalid_file_and_fails() {
    let path = temp_file("cli_invalid.json", "[ 1 2 x\n");

    let output = run(&["-i"], Some(&path));

    assert_eq!(output.status.code(), Some(1));
    assert!(output.stdout.is_empty());
    assert_eq!(std::fs::read_to_string(&path).unwrap(), "[ 1 2 x\n");
}

#[test]
fn in_place_flag_requires_file() {
    let output = run(&["-i"], None);

    assert_eq!(output.status.code(), Some(2));
}

#[test]
fn in_place_flag_rejects_stdin() {
    let output = run(&["-i", "-"], None);

    assert_eq!(output.status.code(), Some(2));
}

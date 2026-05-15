use std::process::{Command, Output};

fn minecli(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_minecli"))
        .args(args)
        .output()
        .expect("failed to run minecli")
}

fn stdout(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).to_string()
}

fn stderr(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).to_string()
}

#[test]
fn init_creates_local_state() {
    let temp = tempfile::tempdir().unwrap();
    let server_path = temp.path().to_string_lossy().to_string();

    let output = minecli(&[
        "--path",
        &server_path,
        "init",
        "--type",
        "fabric",
        "--minecraft",
        "1.21.5",
        "--name",
        "survival",
    ]);

    assert!(
        output.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        stdout(&output),
        stderr(&output)
    );
    assert!(temp.path().join(".minecli/server.toml").exists());
    assert!(temp.path().join(".minecli/lock.toml").exists());
    assert!(temp.path().join(".minecli/history.log").exists());
}

#[test]
fn status_and_list_work_after_init() {
    let temp = tempfile::tempdir().unwrap();
    let server_path = temp.path().to_string_lossy().to_string();
    let init = minecli(&[
        "--path",
        &server_path,
        "init",
        "--type",
        "paper",
        "--minecraft",
        "1.21.5",
        "--name",
        "lobby",
    ]);
    assert!(init.status.success());

    let status = minecli(&["--path", &server_path, "status"]);
    assert!(status.status.success());
    let status_stdout = stdout(&status);
    assert!(status_stdout.contains("Server: lobby"));
    assert!(status_stdout.contains("Type: paper"));

    let list = minecli(&["--path", &server_path, "list"]);
    assert!(list.status.success());
    assert!(stdout(&list).contains("No packages installed."));

    let json_list = minecli(&["--path", &server_path, "list", "--json"]);
    assert!(json_list.status.success());
    assert_eq!(stdout(&json_list).trim(), "[]");
}

#[test]
fn doctor_reports_missing_target_directories() {
    let temp = tempfile::tempdir().unwrap();
    let server_path = temp.path().to_string_lossy().to_string();
    let init = minecli(&[
        "--path",
        &server_path,
        "init",
        "--type",
        "fabric",
        "--minecraft",
        "1.21.5",
    ]);
    assert!(init.status.success());

    let doctor = minecli(&["--path", &server_path, "doctor"]);

    assert!(!doctor.status.success());
    assert!(stdout(&doctor).contains("missing directory: mods"));
}

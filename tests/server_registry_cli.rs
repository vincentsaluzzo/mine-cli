use std::path::Path;
use std::process::{Command, Output};

fn minecli(config_dir: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_minecli"))
        .env("MINECLI_CONFIG_DIR", config_dir)
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

fn assert_success(output: &Output) {
    assert!(
        output.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        stdout(output),
        stderr(output)
    );
}

#[test]
fn registers_lists_shows_and_removes_servers() {
    let temp = tempfile::tempdir().unwrap();
    let config_dir = temp.path().join("config");
    let server_dir = temp.path().join("server");
    std::fs::create_dir(&server_dir).unwrap();

    let add = minecli(
        &config_dir,
        &[
            "servers",
            "add",
            "survival",
            server_dir.to_string_lossy().as_ref(),
        ],
    );
    assert_success(&add);
    assert!(config_dir.join("servers.toml").exists());
    assert!(config_dir.join("config.toml").exists());

    let list = minecli(&config_dir, &["servers", "list"]);
    assert_success(&list);
    assert!(stdout(&list).contains("survival"));
    assert!(stdout(&list).contains(server_dir.to_string_lossy().as_ref()));

    let show = minecli(&config_dir, &["servers", "show", "survival"]);
    assert_success(&show);
    assert!(stdout(&show).contains("Exists: true"));

    let remove = minecli(&config_dir, &["servers", "remove", "survival"]);
    assert_success(&remove);

    let list_after_remove = minecli(&config_dir, &["servers", "list"]);
    assert_success(&list_after_remove);
    assert!(stdout(&list_after_remove).contains("No servers registered."));
}

#[test]
fn rejects_duplicate_server_names() {
    let temp = tempfile::tempdir().unwrap();
    let config_dir = temp.path().join("config");
    let server_dir = temp.path().join("server");
    std::fs::create_dir(&server_dir).unwrap();

    let first = minecli(
        &config_dir,
        &["servers", "add", "survival", server_dir.to_str().unwrap()],
    );
    assert_success(&first);

    let duplicate = minecli(
        &config_dir,
        &["servers", "add", "survival", server_dir.to_str().unwrap()],
    );

    assert!(!duplicate.status.success());
    assert!(stderr(&duplicate).contains("already registered"));
}

#[test]
fn resolves_registered_server_for_regular_commands() {
    let temp = tempfile::tempdir().unwrap();
    let config_dir = temp.path().join("config");
    let server_dir = temp.path().join("server");
    std::fs::create_dir(&server_dir).unwrap();

    let add = minecli(
        &config_dir,
        &["servers", "add", "survival", server_dir.to_str().unwrap()],
    );
    assert_success(&add);

    let init = minecli(
        &config_dir,
        &[
            "--server",
            "survival",
            "init",
            "--type",
            "fabric",
            "--minecraft",
            "1.21.5",
            "--name",
            "survival",
        ],
    );
    assert_success(&init);

    let status = minecli(&config_dir, &["--server", "survival", "status"]);
    assert_success(&status);
    assert!(stdout(&status).contains("Server: survival"));
    assert!(stdout(&status).contains("Type: fabric"));
}

#[test]
fn rejects_missing_server_paths() {
    let temp = tempfile::tempdir().unwrap();
    let config_dir = temp.path().join("config");
    let missing = temp.path().join("missing");

    let add = minecli(
        &config_dir,
        &["servers", "add", "missing", missing.to_str().unwrap()],
    );

    assert!(!add.status.success());
    assert!(stderr(&add).contains("server path does not exist"));
}

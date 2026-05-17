use std::path::Path;
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

fn assert_success(output: &Output) {
    assert!(
        output.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        stdout(output),
        stderr(output)
    );
}

#[test]
fn completions_command_generates_shell_script() {
    let output = minecli(&["completions", "bash"]);

    assert_success(&output);
    assert!(stdout(&output).contains("minecli"));
}

#[test]
fn local_main_workflow_exports_and_syncs() {
    let temp = tempfile::tempdir().unwrap();
    let source = temp.path().join("source");
    let target = temp.path().join("target");
    let file = temp.path().join("example.jar");
    std::fs::create_dir_all(&source).unwrap();
    std::fs::create_dir_all(&target).unwrap();
    std::fs::write(&file, b"example").unwrap();

    assert_success(&minecli(&[
        "--path",
        source.to_str().unwrap(),
        "init",
        "--type",
        "fabric",
        "--minecraft",
        "1.21.5",
        "--name",
        "source",
    ]));
    assert_success(&minecli(&[
        "--path",
        source.to_str().unwrap(),
        "install",
        "--file",
        file.to_str().unwrap(),
        "--kind",
        "mod",
    ]));
    assert_success(&minecli(&[
        "--path",
        target.to_str().unwrap(),
        "sync",
        source.to_str().unwrap(),
    ]));

    assert!(target.join("mods/example.jar").exists());
    let list = minecli(&["--path", target.to_str().unwrap(), "list"]);
    assert_success(&list);
    assert!(stdout(&list).contains("example"));
}

#[test]
fn filesystem_sandbox_rejects_unsafe_manifest_paths() {
    let temp = tempfile::tempdir().unwrap();
    let server = temp.path().join("server");
    std::fs::create_dir_all(server.join(".minecli")).unwrap();
    std::fs::write(
        server.join(".minecli/server.toml"),
        r#"
name = "unsafe"
minecraft_version = "1.21.5"
server_type = "fabric"
world = "world"

[paths]
mods = "../mods"
plugins = "plugins"
datapacks = "world/datapacks"
"#,
    )
    .unwrap();

    let output = minecli(&["--path", as_str(&server), "doctor"]);

    assert!(!output.status.success());
    assert!(stderr(&output).contains("mods path cannot escape"));
}

fn as_str(path: &Path) -> &str {
    path.to_str().unwrap()
}

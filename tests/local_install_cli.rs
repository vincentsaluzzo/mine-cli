use std::path::Path;
use std::process::{Command, Output};

fn minecli(server_dir: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_minecli"))
        .arg("--path")
        .arg(server_dir)
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

fn init_fabric(server_dir: &Path) {
    let output = minecli(
        server_dir,
        &[
            "init",
            "--type",
            "fabric",
            "--minecraft",
            "1.21.5",
            "--name",
            "local",
        ],
    );
    assert_success(&output);
}

#[test]
fn installs_local_file_and_tracks_it() {
    let temp = tempfile::tempdir().unwrap();
    let server_dir = temp.path().join("server");
    let source_file = temp.path().join("example-mod.jar");
    std::fs::create_dir(&server_dir).unwrap();
    std::fs::write(&source_file, b"example mod").unwrap();
    init_fabric(&server_dir);

    let install = minecli(
        &server_dir,
        &[
            "install",
            "--file",
            source_file.to_str().unwrap(),
            "--kind",
            "mod",
        ],
    );
    assert_success(&install);

    assert_eq!(
        std::fs::read(server_dir.join("mods/example-mod.jar")).unwrap(),
        b"example mod"
    );
    let list = minecli(&server_dir, &["list"]);
    assert_success(&list);
    assert!(stdout(&list).contains("example-mod"));
    assert!(stdout(&list).contains("local-file"));
}

#[test]
fn installs_local_folder_files() {
    let temp = tempfile::tempdir().unwrap();
    let server_dir = temp.path().join("server");
    let source_dir = temp.path().join("mods-source");
    std::fs::create_dir(&server_dir).unwrap();
    std::fs::create_dir(&source_dir).unwrap();
    std::fs::write(source_dir.join("a.jar"), b"a").unwrap();
    std::fs::write(source_dir.join("b.jar"), b"b").unwrap();
    std::fs::write(source_dir.join("notes.txt"), b"ignore me").unwrap();
    init_fabric(&server_dir);

    let install = minecli(
        &server_dir,
        &[
            "install",
            "--folder",
            source_dir.to_str().unwrap(),
            "--kind",
            "mod",
        ],
    );
    assert_success(&install);

    assert!(server_dir.join("mods/a.jar").exists());
    assert!(server_dir.join("mods/b.jar").exists());
    assert!(!server_dir.join("mods/notes.txt").exists());
    let list = minecli(&server_dir, &["list"]);
    assert_success(&list);
    assert!(stdout(&list).contains("local-folder"));
}

#[test]
fn dry_run_local_file_does_not_copy() {
    let temp = tempfile::tempdir().unwrap();
    let server_dir = temp.path().join("server");
    let source_file = temp.path().join("example-mod.jar");
    std::fs::create_dir(&server_dir).unwrap();
    std::fs::write(&source_file, b"example mod").unwrap();
    init_fabric(&server_dir);

    let install = Command::new(env!("CARGO_BIN_EXE_minecli"))
        .arg("--path")
        .arg(&server_dir)
        .arg("--dry-run")
        .arg("install")
        .arg("--file")
        .arg(&source_file)
        .arg("--kind")
        .arg("mod")
        .output()
        .expect("failed to run minecli");
    assert_success(&install);

    assert!(stdout(&install).contains("Dry run"));
    assert!(!server_dir.join("mods/example-mod.jar").exists());
}

#[test]
fn rejects_plugin_file_on_fabric_server() {
    let temp = tempfile::tempdir().unwrap();
    let server_dir = temp.path().join("server");
    let source_file = temp.path().join("plugin.jar");
    std::fs::create_dir(&server_dir).unwrap();
    std::fs::write(&source_file, b"plugin").unwrap();
    init_fabric(&server_dir);

    let install = minecli(
        &server_dir,
        &[
            "install",
            "--file",
            source_file.to_str().unwrap(),
            "--kind",
            "plugin",
        ],
    );

    assert!(!install.status.success());
    assert!(stderr(&install).contains("fabric servers cannot install plugin packages"));
}

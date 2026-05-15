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

#[test]
fn remove_leaves_unmanaged_files_alone() {
    let temp = tempfile::tempdir().unwrap();
    let server_dir = temp.path().join("server");
    std::fs::create_dir_all(server_dir.join("mods")).unwrap();
    std::fs::write(server_dir.join("mods/manual.jar"), b"manual").unwrap();
    std::fs::create_dir_all(server_dir.join(".minecli")).unwrap();
    std::fs::write(
        server_dir.join(".minecli/server.toml"),
        r#"
name = "mocked"
minecraft_version = "1.21.5"
server_type = "fabric"
world = "world"

[paths]
mods = "mods"
plugins = "plugins"
datapacks = "world/datapacks"
"#,
    )
    .unwrap();
    std::fs::write(
        server_dir.join(".minecli/lock.toml"),
        r#"
[[packages]]
source = "modrinth"
project_id = "tracked"
slug = "tracked"
title = "tracked"
kind = "mod"
loader = "fabric"
version_id = "tracked-version"
version_number = "1.0.0"
filename = "tracked.jar"
installed_path = "mods/tracked.jar"
dependencies = []
installed_as_dependency = false
"#,
    )
    .unwrap();
    std::fs::write(server_dir.join("mods/tracked.jar"), b"tracked").unwrap();

    let remove = minecli(&server_dir, &["remove", "tracked"]);

    assert_success(&remove);
    assert!(!server_dir.join("mods/tracked.jar").exists());
    assert!(server_dir.join("mods/manual.jar").exists());
}

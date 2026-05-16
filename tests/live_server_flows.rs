use std::fs;
use std::io::Write;
use std::path::Path;
use std::process::{Command, Output};
use std::time::Duration;

use reqwest::blocking::Client;
use serde_json::Value;

const MINECRAFT_VERSION: &str = "1.21.5";

#[derive(Debug, Clone, Copy)]
enum ServerFlavor {
    Fabric,
    Purpur,
    Forge,
    NeoForge,
}

impl ServerFlavor {
    fn expected_type(self) -> &'static str {
        match self {
            Self::Fabric => "fabric",
            Self::Purpur => "purpur",
            Self::Forge => "forge",
            Self::NeoForge => "neoforge",
        }
    }

    fn package(self) -> (&'static str, &'static str, &'static str) {
        match self {
            Self::Fabric => ("fabric-api", "mod", "mods"),
            Self::Purpur => ("simple-voice-chat", "plugin", "plugins"),
            Self::Forge | Self::NeoForge => ("simple-voice-chat", "mod", "mods"),
        }
    }
}

fn minecli(server_dir: &Path, cache_dir: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_minecli"))
        .env("MINECLI_CACHE_DIR", cache_dir)
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

#[ignore = "downloads real server jars and packages from external sources"]
#[test]
fn live_fabric_server_flow() {
    run_live_flow(ServerFlavor::Fabric);
}

#[ignore = "downloads real server jars and packages from external sources"]
#[test]
fn live_purpur_server_flow() {
    run_live_flow(ServerFlavor::Purpur);
}

#[ignore = "downloads real server jars and packages from external sources"]
#[test]
fn live_forge_server_flow() {
    run_live_flow(ServerFlavor::Forge);
}

#[ignore = "downloads real server jars and packages from external sources"]
#[test]
fn live_neoforge_server_flow() {
    run_live_flow(ServerFlavor::NeoForge);
}

fn run_live_flow(flavor: ServerFlavor) {
    let workspace = tempfile::tempdir().unwrap();
    let server_dir = workspace.path().join("server");
    let cache_dir = workspace.path().join("cache");
    fs::create_dir_all(&server_dir).unwrap();

    log_step(flavor, format!("workspace: {}", workspace.path().display()));
    log_step(flavor, format!("server dir: {}", server_dir.display()));
    log_step(flavor, format!("cache dir: {}", cache_dir.display()));

    let client = http_client();
    log_step(flavor, "downloading server artifact");
    download_server_jar(&client, flavor, &server_dir);
    log_directory(flavor, "server files after download", &server_dir);

    log_step(flavor, "running minecli init with auto-detection");
    let init = minecli(
        &server_dir,
        &cache_dir,
        &["init", "--minecraft", MINECRAFT_VERSION, "--name", "live"],
    );
    assert_success(&init);
    log_output(flavor, "init stdout", &stdout(&init));

    log_step(flavor, "checking detected server type with minecli status");
    let status = minecli(&server_dir, &cache_dir, &["status"]);
    assert_success(&status);
    log_output(flavor, "status stdout", &stdout(&status));
    assert!(
        stdout(&status).contains(&format!("Type: {}", flavor.expected_type())),
        "{}",
        stdout(&status)
    );

    let (project, kind, target_dir) = flavor.package();
    log_step(
        flavor,
        format!("installing package `{project}` as `{kind}`"),
    );
    let install = minecli(
        &server_dir,
        &cache_dir,
        &["install", project, "--kind", kind],
    );
    assert_success(&install);
    log_output(flavor, "install stdout", &stdout(&install));
    log_directory(
        flavor,
        format!("target directory after install: {target_dir}"),
        &server_dir.join(target_dir),
    );
    log_directory(flavor, "download cache after install", &cache_dir);

    log_step(flavor, "checking lockfile list");
    let list = minecli(&server_dir, &cache_dir, &["list"]);
    assert_success(&list);
    log_output(flavor, "list stdout", &stdout(&list));
    assert!(stdout(&list).contains(project), "{}", stdout(&list));
    assert!(
        installed_files(&server_dir.join(target_dir)) > 0,
        "expected at least one file in {target_dir}"
    );

    log_step(flavor, format!("removing `{project}`"));
    let remove = minecli(
        &server_dir,
        &cache_dir,
        &["remove", project, "--remove-orphans"],
    );
    assert_success(&remove);
    log_output(flavor, "remove stdout", &stdout(&remove));
    log_directory(
        flavor,
        format!("target directory after remove: {target_dir}"),
        &server_dir.join(target_dir),
    );

    let list_after_remove = minecli(&server_dir, &cache_dir, &["list", "--json"]);
    assert_success(&list_after_remove);
    log_output(
        flavor,
        "list --json stdout after remove",
        &stdout(&list_after_remove),
    );
    assert_eq!(stdout(&list_after_remove).trim(), "[]");
    assert_eq!(installed_files(&server_dir.join(target_dir)), 0);
    log_step(flavor, "flow completed");
}

fn http_client() -> Client {
    Client::builder()
        .timeout(Duration::from_secs(120))
        .user_agent(format!(
            "minecli-live-tests/{} (github.com/vincentsaluzzo/mine-cli)",
            env!("CARGO_PKG_VERSION")
        ))
        .build()
        .unwrap()
}

fn download_server_jar(client: &Client, flavor: ServerFlavor, server_dir: &Path) {
    match flavor {
        ServerFlavor::Fabric => download_fabric_server(client, server_dir),
        ServerFlavor::Purpur => download_purpur_server(client, server_dir),
        ServerFlavor::Forge => download_forge_installer(client, server_dir),
        ServerFlavor::NeoForge => download_neoforge_installer(client, server_dir),
    }
}

fn download_fabric_server(client: &Client, server_dir: &Path) {
    let loader_version = latest_fabric_loader(client);
    let installer_version = latest_fabric_installer(client);
    let url = format!(
        "https://meta.fabricmc.net/v2/versions/loader/{MINECRAFT_VERSION}/{loader_version}/{installer_version}/server/jar"
    );
    println!("[fabric] loader={loader_version}, installer={installer_version}");
    download_to(client, &url, &server_dir.join("fabric-server-launch.jar"));
}

fn latest_fabric_loader(client: &Client) -> String {
    let versions = client
        .get(format!(
            "https://meta.fabricmc.net/v2/versions/loader/{MINECRAFT_VERSION}"
        ))
        .send()
        .unwrap()
        .error_for_status()
        .unwrap()
        .json::<Vec<Value>>()
        .unwrap();

    versions
        .iter()
        .find(|version| version["loader"]["stable"].as_bool().unwrap_or(false))
        .or_else(|| versions.first())
        .and_then(|version| version["loader"]["version"].as_str())
        .expect("fabric loader version")
        .to_owned()
}

fn latest_fabric_installer(client: &Client) -> String {
    let versions = client
        .get("https://meta.fabricmc.net/v2/versions/installer")
        .send()
        .unwrap()
        .error_for_status()
        .unwrap()
        .json::<Vec<Value>>()
        .unwrap();

    versions
        .iter()
        .find(|version| version["stable"].as_bool().unwrap_or(false))
        .or_else(|| versions.first())
        .and_then(|version| version["version"].as_str())
        .expect("fabric installer version")
        .to_owned()
}

fn download_purpur_server(client: &Client, server_dir: &Path) {
    let latest = client
        .get(format!(
            "https://api.purpurmc.org/v2/purpur/{MINECRAFT_VERSION}/latest"
        ))
        .send()
        .unwrap()
        .error_for_status()
        .unwrap()
        .json::<Value>()
        .unwrap();
    let build = latest["build"].as_str().expect("purpur build");
    let url = format!("https://api.purpurmc.org/v2/purpur/{MINECRAFT_VERSION}/{build}/download");
    println!("[purpur] build={build}");
    download_to(
        client,
        &url,
        &server_dir.join(format!("purpur-{MINECRAFT_VERSION}.jar")),
    );
}

fn download_forge_installer(client: &Client, server_dir: &Path) {
    let metadata = download_text(
        client,
        "https://maven.minecraftforge.net/net/minecraftforge/forge/maven-metadata.xml",
    );
    let version = latest_version_from_maven_metadata(&metadata, &format!("{MINECRAFT_VERSION}-"));
    let url = format!(
        "https://maven.minecraftforge.net/net/minecraftforge/forge/{version}/forge-{version}-installer.jar"
    );
    println!("[forge] installer version={version}");
    download_to(
        client,
        &url,
        &server_dir.join(format!("forge-{version}-installer.jar")),
    );
}

fn download_neoforge_installer(client: &Client, server_dir: &Path) {
    let metadata = download_text(
        client,
        "https://maven.neoforged.net/releases/net/neoforged/neoforge/maven-metadata.xml",
    );
    let version = latest_version_from_maven_metadata(&metadata, "21.5.");
    let url = format!(
        "https://maven.neoforged.net/releases/net/neoforged/neoforge/{version}/neoforge-{version}-installer.jar"
    );
    println!("[neoforge] installer version={version}");
    download_to(
        client,
        &url,
        &server_dir.join(format!("neoforge-{version}-installer.jar")),
    );
}

fn latest_version_from_maven_metadata(metadata: &str, prefix: &str) -> String {
    metadata
        .lines()
        .filter_map(|line| {
            line.trim()
                .strip_prefix("<version>")
                .and_then(|value| value.strip_suffix("</version>"))
        })
        .rfind(|version| version.starts_with(prefix))
        .expect("matching maven version")
        .to_owned()
}

fn download_text(client: &Client, url: &str) -> String {
    client
        .get(url)
        .send()
        .unwrap()
        .error_for_status()
        .unwrap()
        .text()
        .unwrap()
}

fn download_to(client: &Client, url: &str, path: &Path) {
    println!("  download: {url}");
    println!("       to: {}", path.display());
    let mut response = client.get(url).send().unwrap().error_for_status().unwrap();
    let mut file = fs::File::create(path).unwrap();
    let bytes = response.copy_to(&mut file).unwrap();
    file.flush().unwrap();
    println!("  wrote {bytes} bytes");
}

fn installed_files(path: &Path) -> usize {
    match fs::read_dir(path) {
        Ok(entries) => entries
            .filter_map(Result::ok)
            .filter(|entry| entry.path().is_file())
            .count(),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => 0,
        Err(error) => panic!("failed to read {}: {error}", path.display()),
    }
}

fn log_step(flavor: ServerFlavor, message: impl AsRef<str>) {
    println!("[{}] {}", flavor.expected_type(), message.as_ref());
}

fn log_output(flavor: ServerFlavor, title: &str, output: &str) {
    log_step(flavor, title);
    for line in output.trim().lines() {
        println!("  {line}");
    }
}

fn log_directory(flavor: ServerFlavor, title: impl AsRef<str>, path: &Path) {
    log_step(flavor, title);
    match fs::read_dir(path) {
        Ok(entries) => {
            let mut entries = entries
                .filter_map(Result::ok)
                .map(|entry| {
                    let path = entry.path();
                    let size = fs::metadata(&path)
                        .map(|metadata| metadata.len())
                        .unwrap_or(0);
                    let name = entry.file_name().to_string_lossy().to_string();
                    (name, size)
                })
                .collect::<Vec<_>>();
            entries.sort_by(|left, right| left.0.cmp(&right.0));

            if entries.is_empty() {
                println!("  <empty>");
            } else {
                for (name, size) in entries {
                    println!("  {name} ({size} bytes)");
                }
            }
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            println!("  <missing>");
        }
        Err(error) => panic!("failed to read {}: {error}", path.display()),
    }
}

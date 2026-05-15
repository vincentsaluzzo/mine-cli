use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

use reqwest::blocking::Client;

use crate::cli::{Command, GlobalOptions};
use crate::core::history;
use crate::core::lockfile::{LockFile, LockedPackage, load_lockfile, write_lockfile};
use crate::core::manifest::{ServerConfig, minecli_dir, server_file};
use crate::core::manifest::{load_server_config, write_server_config};
use crate::core::server::{
    ContentKind, ServerType, content_kind_from_project_type, detect_server_type, detect_world_name,
};
use crate::error::{IoResultExt, MinecliError, Result};
use crate::fsops::{cache_dir, copy_verified_download, verify_file_hash};
use crate::sources::modrinth::{
    DependencyType, ModrinthClient, ModrinthFile, ProjectSource, ReleaseChannel, SearchParams,
    select_version, version_matches_server,
};

pub fn execute(globals: GlobalOptions, command: Command) -> Result<()> {
    if globals.verbose {
        eprintln!("server path: {}", globals.server_dir.display());
        if let Some(config) = &globals.config {
            eprintln!("config override: {}", config.display());
        }
        if globals.yes {
            eprintln!("automatic yes enabled");
        }
    }

    match command {
        Command::Init {
            server_type,
            minecraft,
            name,
            force,
        } => init(&globals, server_type, minecraft, name, force),
        Command::Status => status(&globals),
        Command::Search { query, kind, limit } => search(&globals, query, kind, limit),
        Command::Install {
            project,
            kind,
            version,
            channel,
            no_deps,
        } => install(&globals, project, kind, version, channel, no_deps),
        Command::List { kind, json } => list(&globals, kind, json),
        Command::Remove {
            project,
            remove_orphans,
        } => remove(&globals, project, remove_orphans),
        Command::Doctor => doctor(&globals),
    }
}

fn init(
    globals: &GlobalOptions,
    server_type: Option<ServerType>,
    minecraft_version: String,
    name: Option<String>,
    force: bool,
) -> Result<()> {
    let server_dir = &globals.server_dir;
    fs::create_dir_all(server_dir).at(server_dir)?;

    let minecli_path = minecli_dir(server_dir);
    if minecli_path.exists() && !force {
        return Err(MinecliError::message(format!(
            "{} already exists; pass --force to reinitialize",
            minecli_path.display()
        )));
    }

    let detected_type = match server_type {
        Some(server_type) => server_type,
        None => detect_server_type(server_dir)?,
    };
    let world = detect_world_name(server_dir)?;
    let name = name.unwrap_or_else(|| {
        server_dir
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("minecraft-server")
            .to_owned()
    });

    let config = ServerConfig::new(name, minecraft_version, detected_type, world);
    write_server_config(server_dir, &config)?;
    write_lockfile(server_dir, &LockFile::default())?;
    history::record(server_dir, "init")?;

    println!(
        "Initialized {} as a {} server for Minecraft {}",
        server_file(server_dir).display(),
        config.server_type,
        config.minecraft_version
    );
    Ok(())
}

fn status(globals: &GlobalOptions) -> Result<()> {
    let config = load_server_config(&globals.server_dir)?;
    let lockfile = load_lockfile(&globals.server_dir)?;
    let unmanaged = unmanaged_files(&globals.server_dir, &config, &lockfile)?;

    let mods = lockfile
        .packages
        .iter()
        .filter(|package| package.kind == ContentKind::Mod)
        .count();
    let plugins = lockfile
        .packages
        .iter()
        .filter(|package| package.kind == ContentKind::Plugin)
        .count();
    let datapacks = lockfile
        .packages
        .iter()
        .filter(|package| package.kind == ContentKind::Datapack)
        .count();

    println!("Server: {}", config.name);
    println!("Minecraft: {}", config.minecraft_version);
    println!("Type: {}", config.server_type);
    println!("Packages: {mods} mods, {plugins} plugins, {datapacks} datapacks");
    println!("Unmanaged files: {}", unmanaged.len());
    for file in unmanaged {
        println!("  {}", file.display());
    }

    Ok(())
}

fn search(
    globals: &GlobalOptions,
    query: String,
    kind: Option<ContentKind>,
    limit: usize,
) -> Result<()> {
    let context = optional_server_context(&globals.server_dir)?;
    let params = SearchParams::for_server(
        query,
        context
            .as_ref()
            .map(|config| config.minecraft_version.clone()),
        context.as_ref().map(|config| config.server_type),
        kind,
        limit,
    );
    let client = ModrinthClient::new()?;
    let response = client.search(&params)?;

    if response.hits.is_empty() {
        println!("No matching Modrinth projects found.");
        return Ok(());
    }

    println!(
        "{:<24} {:<10} {:<12} {:<12} {:>10} Title",
        "Slug", "Kind", "Server", "Client", "Downloads"
    );
    for hit in response.hits {
        println!(
            "{:<24} {:<10} {:<12} {:<12} {:>10} {}",
            truncate(&hit.slug, 24),
            hit.project_type,
            hit.server_side,
            hit.client_side,
            hit.downloads,
            hit.title
        );
        println!("  {} | {}", hit.project_id, hit.description);
    }

    Ok(())
}

fn install(
    globals: &GlobalOptions,
    project: String,
    kind: Option<ContentKind>,
    requested_version: Option<String>,
    channel: ReleaseChannel,
    no_deps: bool,
) -> Result<()> {
    let config = load_server_config(&globals.server_dir)?;
    let mut lockfile = load_lockfile(&globals.server_dir)?;
    let client = ModrinthClient::new()?;
    let mut resolver = InstallResolver::new(&client, &config, &lockfile, !no_deps, channel);
    let plan = resolver.resolve(&project, kind, requested_version.as_deref(), false)?;

    if plan.is_empty() {
        println!("{project} is already installed with the selected version.");
        return Ok(());
    }

    validate_install_plan(&globals.server_dir, &lockfile, &plan)?;
    print_install_plan(&plan, globals.dry_run);
    if globals.dry_run {
        return Ok(());
    }

    let cache = cache_dir()?;
    apply_install_plan(
        &globals.server_dir,
        &mut lockfile,
        client.http_client(),
        &cache,
        plan,
    )?;
    write_lockfile(&globals.server_dir, &lockfile)?;
    history::record(&globals.server_dir, format!("install {project}"))?;
    println!("Install complete.");
    Ok(())
}

fn list(globals: &GlobalOptions, kind: Option<ContentKind>, json: bool) -> Result<()> {
    let lockfile = load_lockfile(&globals.server_dir)?;
    let packages = lockfile
        .packages
        .iter()
        .filter(|package| kind.is_none_or(|kind| package.kind == kind))
        .collect::<Vec<_>>();

    if json {
        println!("{}", serde_json::to_string_pretty(&packages)?);
        return Ok(());
    }

    if packages.is_empty() {
        println!("No packages installed.");
        return Ok(());
    }

    println!(
        "{:<24} {:<10} {:<16} {:<10} Path",
        "Slug", "Kind", "Version", "Source"
    );
    for package in packages {
        println!(
            "{:<24} {:<10} {:<16} {:<10} {}",
            truncate(&package.slug, 24),
            package.kind,
            truncate(&package.version_number, 16),
            package.source,
            package.installed_path.display()
        );
    }
    Ok(())
}

fn remove(globals: &GlobalOptions, project: String, remove_orphans: bool) -> Result<()> {
    let mut lockfile = load_lockfile(&globals.server_dir)?;
    let package = lockfile
        .package_by_query(&project)
        .cloned()
        .ok_or_else(|| MinecliError::message(format!("package `{project}` is not installed")))?;

    let dependents = lockfile
        .packages
        .iter()
        .filter(|candidate| candidate.dependencies.contains(&package.project_id))
        .map(|candidate| candidate.slug.clone())
        .collect::<Vec<_>>();

    if !dependents.is_empty() {
        println!(
            "Warning: {} is required by {}",
            package.slug,
            dependents.join(", ")
        );
    }

    let mut to_remove = vec![package.project_id.clone()];
    if remove_orphans {
        collect_orphan_dependencies(&lockfile, &mut to_remove);
    }

    println!("Remove plan:");
    for project_id in &to_remove {
        if let Some(package) = lockfile.package_by_project_id(project_id) {
            println!(
                "  - {} ({})",
                package.slug,
                package.installed_path.display()
            );
        }
    }

    if globals.dry_run {
        println!("Dry run: no files changed.");
        return Ok(());
    }

    for project_id in to_remove {
        if let Some(package) = lockfile.remove_project(&project_id) {
            let path = globals.server_dir.join(&package.installed_path);
            match fs::remove_file(&path) {
                Ok(()) => {}
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                Err(error) => {
                    return Err(MinecliError::Io {
                        path,
                        source: error,
                    });
                }
            }
        }
    }

    write_lockfile(&globals.server_dir, &lockfile)?;
    history::record(&globals.server_dir, format!("remove {project}"))?;
    println!("Remove complete.");
    Ok(())
}

fn doctor(globals: &GlobalOptions) -> Result<()> {
    let config = load_server_config(&globals.server_dir)?;
    let lockfile = load_lockfile(&globals.server_dir)?;
    let mut issues = Vec::new();

    for path in [
        &config.paths.mods,
        &config.paths.plugins,
        &config.paths.datapacks,
    ] {
        let absolute = globals.server_dir.join(path);
        if !absolute.exists() {
            issues.push(format!("missing directory: {}", path.display()));
        }
    }

    for package in &lockfile.packages {
        let path = globals.server_dir.join(&package.installed_path);
        if !path.exists() {
            issues.push(format!(
                "missing installed file for {}: {}",
                package.slug,
                package.installed_path.display()
            ));
            continue;
        }

        if let Err(error) = verify_file_hash(&path, &package.hashes, &package.filename) {
            issues.push(format!("{error}"));
        }
    }

    if issues.is_empty() {
        println!("No issues found.");
        return Ok(());
    }

    println!("Found {} issue(s):", issues.len());
    for issue in &issues {
        println!("  - {issue}");
    }

    Err(MinecliError::message("doctor found issues"))
}

#[derive(Debug, Clone)]
struct ServerContext {
    minecraft_version: String,
    server_type: ServerType,
}

fn optional_server_context(server_dir: &Path) -> Result<Option<ServerContext>> {
    let path = server_file(server_dir);
    if !path.exists() {
        return Ok(None);
    }
    let config = load_server_config(server_dir)?;
    Ok(Some(ServerContext {
        minecraft_version: config.minecraft_version,
        server_type: config.server_type,
    }))
}

#[derive(Debug, Clone)]
pub(crate) struct PlannedInstall {
    version_name: String,
    pub(crate) locked_package: LockedPackage,
    pub(crate) file: ModrinthFile,
    installed_path: PathBuf,
}

pub(crate) struct InstallResolver<'a, S: ProjectSource> {
    client: &'a S,
    config: &'a ServerConfig,
    lockfile: &'a LockFile,
    include_dependencies: bool,
    channel: ReleaseChannel,
    visiting: HashSet<String>,
    planned_project_ids: HashSet<String>,
}

impl<'a, S: ProjectSource> InstallResolver<'a, S> {
    fn new(
        client: &'a S,
        config: &'a ServerConfig,
        lockfile: &'a LockFile,
        include_dependencies: bool,
        channel: ReleaseChannel,
    ) -> Self {
        Self {
            client,
            config,
            lockfile,
            include_dependencies,
            channel,
            visiting: HashSet::new(),
            planned_project_ids: HashSet::new(),
        }
    }

    fn resolve(
        &mut self,
        project_ref: &str,
        requested_kind: Option<ContentKind>,
        requested_version: Option<&str>,
        installed_as_dependency: bool,
    ) -> Result<Vec<PlannedInstall>> {
        let project = self.client.get_project(project_ref)?;
        if project.server_side == "unsupported" {
            return Err(MinecliError::message(format!(
                "{} is marked as unsupported on servers by Modrinth",
                project.slug
            )));
        }
        if project.client_side == "required" && project.server_side != "required" {
            println!(
                "Warning: {} is client-side required and may not be useful on a server.",
                project.slug
            );
        }

        let kind = requested_kind
            .map(Ok)
            .unwrap_or_else(|| content_kind_from_project_type(&project.project_type))?;

        if !self.config.server_type.supports(kind) {
            return Err(MinecliError::message(format!(
                "{} servers cannot install {kind} projects",
                self.config.server_type
            )));
        }

        if self.planned_project_ids.contains(&project.id) {
            return Ok(Vec::new());
        }
        if !self.visiting.insert(project.id.clone()) {
            return Err(MinecliError::message(format!(
                "dependency cycle detected at {}",
                project.slug
            )));
        }

        let loader = self
            .config
            .server_type
            .modrinth_loader(kind)
            .map(ToOwned::to_owned);
        let versions = self.client.get_project_versions(
            &project.id,
            &loader.iter().cloned().collect::<Vec<_>>(),
            std::slice::from_ref(&self.config.minecraft_version),
        )?;
        let version = select_version(&versions, requested_version, self.channel)
            .ok_or_else(|| {
                MinecliError::message(format!(
                    "no compatible {} version found for Minecraft {}{}",
                    project.slug,
                    self.config.minecraft_version,
                    loader
                        .as_ref()
                        .map(|loader| format!(" and loader {loader}"))
                        .unwrap_or_default()
                ))
            })?
            .clone();
        debug_assert_eq!(version.project_id, project.id);

        let mut plan = Vec::new();
        let mut required_dependency_ids = Vec::new();
        if self.include_dependencies {
            for dependency in &version.dependencies {
                match dependency.dependency_type {
                    DependencyType::Required => {
                        if let Some(dependency_project_id) = &dependency.project_id {
                            let dependency_plan = self.resolve_dependency(
                                dependency_project_id,
                                dependency.version_id.as_deref(),
                            )?;
                            required_dependency_ids.push(dependency_project_id.clone());
                            plan.extend(dependency_plan);
                        }
                    }
                    DependencyType::Optional => {
                        println!("Optional dependency available for {}.", project.slug);
                    }
                    DependencyType::Incompatible => {
                        println!(
                            "Warning: {} declares an incompatible dependency.",
                            project.slug
                        );
                    }
                    DependencyType::Embedded => {}
                }
            }
        }

        if self
            .lockfile
            .package_by_project_id(&project.id)
            .is_some_and(|package| package.version_id == version.id)
        {
            self.visiting.remove(&project.id);
            self.planned_project_ids.insert(project.id);
            return Ok(plan);
        }

        let file = version
            .primary_file()
            .ok_or_else(|| {
                MinecliError::message(format!("{} has no downloadable file", project.slug))
            })?
            .clone();
        let target_dir = self.config.paths.target_for(kind);
        let installed_path = target_dir.join(&file.filename);
        let locked_package = LockedPackage {
            source: "modrinth".to_owned(),
            project_id: project.id.clone(),
            slug: project.slug.clone(),
            title: project.title.clone(),
            kind,
            loader,
            version_id: version.id.clone(),
            version_number: version.version_number.clone(),
            filename: file.filename.clone(),
            hashes: file.hashes.clone(),
            installed_path: installed_path.clone(),
            dependencies: required_dependency_ids,
            installed_as_dependency,
        };

        plan.push(PlannedInstall {
            version_name: version.name.clone(),
            locked_package,
            file,
            installed_path,
        });

        self.visiting.remove(&project.id);
        self.planned_project_ids.insert(project.id);
        Ok(plan)
    }

    fn resolve_dependency(
        &mut self,
        dependency_project_id: &str,
        dependency_version_id: Option<&str>,
    ) -> Result<Vec<PlannedInstall>> {
        if let Some(version_id) = dependency_version_id {
            let version = self.client.get_version(version_id)?;
            let project = self.client.get_project(dependency_project_id)?;
            let kind = content_kind_from_project_type(&project.project_type)?;
            let loader = self.config.server_type.modrinth_loader(kind);
            if version_matches_server(&version, &self.config.minecraft_version, loader) {
                return self.resolve(
                    dependency_project_id,
                    Some(kind),
                    Some(&version.version_number),
                    true,
                );
            }
        }

        self.resolve(dependency_project_id, None, None, true)
    }
}

fn print_install_plan(plan: &[PlannedInstall], dry_run: bool) {
    println!("Install plan:");
    for item in plan {
        println!(
            "  + {} {} ({}) -> {} [{} bytes]",
            item.locked_package.slug,
            item.locked_package.version_number,
            item.version_name,
            item.installed_path.display(),
            item.file.size
        );
    }
    if dry_run {
        println!("Dry run: no files changed.");
    }
}

pub(crate) fn apply_install_plan(
    server_dir: &Path,
    lockfile: &mut LockFile,
    client: &Client,
    cache: &Path,
    plan: Vec<PlannedInstall>,
) -> Result<()> {
    for item in plan {
        let cache_path = copy_verified_download(client, &item.file, cache)?;
        let target_path = server_dir.join(&item.installed_path);
        if let Some(parent) = target_path.parent() {
            fs::create_dir_all(parent).at(parent)?;
        }
        fs::copy(&cache_path, &target_path).at(&target_path)?;
        lockfile.upsert_package(item.locked_package);
    }

    Ok(())
}

fn validate_install_plan(
    server_dir: &Path,
    lockfile: &LockFile,
    plan: &[PlannedInstall],
) -> Result<()> {
    for item in plan {
        let target_path = server_dir.join(&item.installed_path);
        if !target_path.exists() {
            continue;
        }

        let tracked_same_package = lockfile.packages.iter().any(|package| {
            package.project_id == item.locked_package.project_id
                && package.installed_path == item.installed_path
        });
        if !tracked_same_package {
            return Err(MinecliError::message(format!(
                "install target already exists and is not owned by this package: {}",
                item.installed_path.display()
            )));
        }
    }

    Ok(())
}

fn unmanaged_files(
    server_dir: &Path,
    config: &ServerConfig,
    lockfile: &LockFile,
) -> Result<Vec<PathBuf>> {
    let managed = lockfile
        .packages
        .iter()
        .map(|package| package.installed_path.clone())
        .collect::<HashSet<_>>();
    let mut unmanaged = Vec::new();

    for directory in [
        &config.paths.mods,
        &config.paths.plugins,
        &config.paths.datapacks,
    ] {
        let absolute = server_dir.join(directory);
        if !absolute.exists() {
            continue;
        }
        for entry in fs::read_dir(&absolute).at(&absolute)? {
            let entry = entry.at(&absolute)?;
            let path = entry.path();
            if !path.is_file() {
                continue;
            }
            let relative = directory.join(entry.file_name());
            if !managed.contains(&relative) {
                unmanaged.push(relative);
            }
        }
    }

    unmanaged.sort();
    Ok(unmanaged)
}

fn collect_orphan_dependencies(lockfile: &LockFile, to_remove: &mut Vec<String>) {
    loop {
        let mut added = false;
        for package in &lockfile.packages {
            if !package.installed_as_dependency || to_remove.contains(&package.project_id) {
                continue;
            }
            let still_needed = lockfile.packages.iter().any(|candidate| {
                !to_remove.contains(&candidate.project_id)
                    && candidate.dependencies.contains(&package.project_id)
            });
            if !still_needed {
                to_remove.push(package.project_id.clone());
                added = true;
            }
        }
        if !added {
            break;
        }
    }
}

fn truncate(value: &str, max_chars: usize) -> String {
    if value.chars().count() <= max_chars {
        return value.to_owned();
    }
    let mut output = value
        .chars()
        .take(max_chars.saturating_sub(1))
        .collect::<String>();
    output.push('~');
    output
}

#[cfg(test)]
mod tests {
    use std::collections::{BTreeMap, HashMap};
    use std::path::PathBuf;

    use sha2::{Digest, Sha512};

    use crate::commands::{
        InstallResolver, PlannedInstall, apply_install_plan, collect_orphan_dependencies,
    };
    use crate::core::lockfile::{LockFile, LockedPackage};
    use crate::core::manifest::ServerConfig;
    use crate::core::server::ContentKind;
    use crate::core::server::ServerType;
    use crate::sources::modrinth::{
        DependencyType, ModrinthFile, Project, ProjectSource, ProjectVersion, ReleaseChannel,
        VersionDependency,
    };

    fn package(project_id: &str, dependencies: Vec<String>, as_dependency: bool) -> LockedPackage {
        LockedPackage {
            source: "modrinth".to_owned(),
            project_id: project_id.to_owned(),
            slug: project_id.to_owned(),
            title: project_id.to_owned(),
            kind: ContentKind::Mod,
            loader: Some("fabric".to_owned()),
            version_id: "version".to_owned(),
            version_number: "1.0.0".to_owned(),
            filename: format!("{project_id}.jar"),
            hashes: BTreeMap::new(),
            installed_path: PathBuf::from(format!("mods/{project_id}.jar")),
            dependencies,
            installed_as_dependency: as_dependency,
        }
    }

    #[test]
    fn collects_orphan_dependencies() {
        let lockfile = LockFile {
            packages: vec![
                package("root", vec!["dep".to_owned()], false),
                package("dep", vec![], true),
            ],
        };
        let mut to_remove = vec!["root".to_owned()];

        collect_orphan_dependencies(&lockfile, &mut to_remove);

        assert_eq!(to_remove, vec!["root", "dep"]);
    }

    #[test]
    fn install_resolver_includes_required_dependencies_first() {
        let source = MockSource::new()
            .with_project(project("root"))
            .with_project(project("dep"))
            .with_versions(
                "root",
                vec![version(
                    "root-version",
                    "root",
                    "1.0.0",
                    vec![required_dependency("dep")],
                )],
            )
            .with_versions("dep", vec![version("dep-version", "dep", "1.0.0", vec![])]);
        let config = config();
        let lockfile = LockFile::default();
        let mut resolver =
            InstallResolver::new(&source, &config, &lockfile, true, ReleaseChannel::Release);

        let plan = resolver
            .resolve("root", Some(ContentKind::Mod), None, false)
            .unwrap();

        assert_eq!(slugs(&plan), vec!["dep", "root"]);
        assert!(plan[0].locked_package.installed_as_dependency);
        assert_eq!(plan[1].locked_package.dependencies, vec!["dep"]);
    }

    #[test]
    fn install_resolver_returns_empty_plan_when_same_version_is_installed() {
        let source = MockSource::new()
            .with_project(project("root"))
            .with_versions(
                "root",
                vec![version("root-version", "root", "1.0.0", vec![])],
            );
        let config = config();
        let mut lockfile = LockFile::default();
        let mut installed = package("root", vec![], false);
        installed.version_id = "root-version".to_owned();
        lockfile.upsert_package(installed);
        let mut resolver =
            InstallResolver::new(&source, &config, &lockfile, true, ReleaseChannel::Release);

        let plan = resolver
            .resolve("root", Some(ContentKind::Mod), None, false)
            .unwrap();

        assert!(plan.is_empty());
    }

    #[test]
    fn install_plan_rejects_unmanaged_file_conflicts() {
        let temp = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(temp.path().join("mods")).unwrap();
        std::fs::write(temp.path().join("mods/root.jar"), b"manual").unwrap();
        let plan = vec![PlannedInstall {
            version_name: "Root 1.0.0".to_owned(),
            locked_package: package("root", vec![], false),
            file: file("root.jar"),
            installed_path: PathBuf::from("mods/root.jar"),
        }];

        let result = super::validate_install_plan(temp.path(), &LockFile::default(), &plan);

        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("install target already exists")
        );
    }

    #[test]
    fn install_resolver_detects_dependency_cycles() {
        let source = MockSource::new()
            .with_project(project("root"))
            .with_project(project("dep"))
            .with_versions(
                "root",
                vec![version(
                    "root-version",
                    "root",
                    "1.0.0",
                    vec![required_dependency("dep")],
                )],
            )
            .with_versions(
                "dep",
                vec![version(
                    "dep-version",
                    "dep",
                    "1.0.0",
                    vec![required_dependency("root")],
                )],
            );
        let config = config();
        let lockfile = LockFile::default();
        let mut resolver =
            InstallResolver::new(&source, &config, &lockfile, true, ReleaseChannel::Release);

        let result = resolver.resolve("root", Some(ContentKind::Mod), None, false);

        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("dependency cycle"));
    }

    #[test]
    fn applies_install_plan_with_mocked_file_downloads() {
        let temp = tempfile::tempdir().unwrap();
        let server_dir = temp.path().join("server");
        let cache_dir = temp.path().join("cache").join("downloads");
        let fixtures_dir = temp.path().join("fixtures");
        std::fs::create_dir_all(&fixtures_dir).unwrap();
        let dep_fixture = fixtures_dir.join("dep.jar");
        let root_fixture = fixtures_dir.join("root.jar");
        std::fs::write(&dep_fixture, b"dep-jar").unwrap();
        std::fs::write(&root_fixture, b"root-jar").unwrap();

        let plan = vec![
            PlannedInstall {
                version_name: "Dep 1.0.0".to_owned(),
                locked_package: package("dep", vec![], true),
                file: file_from_path("dep.jar", &dep_fixture),
                installed_path: PathBuf::from("mods/dep.jar"),
            },
            PlannedInstall {
                version_name: "Root 1.0.0".to_owned(),
                locked_package: package("root", vec!["dep".to_owned()], false),
                file: file_from_path("root.jar", &root_fixture),
                installed_path: PathBuf::from("mods/root.jar"),
            },
        ];
        let mut lockfile = LockFile::default();
        let client = reqwest::blocking::Client::new();

        apply_install_plan(&server_dir, &mut lockfile, &client, &cache_dir, plan).unwrap();

        assert_eq!(
            std::fs::read(server_dir.join("mods/dep.jar")).unwrap(),
            b"dep-jar"
        );
        assert_eq!(
            std::fs::read(server_dir.join("mods/root.jar")).unwrap(),
            b"root-jar"
        );
        assert!(cache_dir.exists());
        assert_eq!(lockfile.packages.len(), 2);
        assert!(lockfile.package_by_project_id("dep").is_some());
        assert!(lockfile.package_by_project_id("root").is_some());
    }

    #[derive(Debug, Default)]
    struct MockSource {
        projects: HashMap<String, Project>,
        versions: HashMap<String, Vec<ProjectVersion>>,
        versions_by_id: HashMap<String, ProjectVersion>,
    }

    impl MockSource {
        fn new() -> Self {
            Self::default()
        }

        fn with_project(mut self, project: Project) -> Self {
            self.projects.insert(project.id.clone(), project);
            self
        }

        fn with_versions(mut self, project_id: &str, versions: Vec<ProjectVersion>) -> Self {
            for version in &versions {
                self.versions_by_id
                    .insert(version.id.clone(), version.clone());
            }
            self.versions.insert(project_id.to_owned(), versions);
            self
        }
    }

    impl ProjectSource for MockSource {
        fn get_project(&self, project: &str) -> crate::error::Result<Project> {
            self.projects
                .get(project)
                .cloned()
                .ok_or_else(|| crate::error::MinecliError::message(format!("missing {project}")))
        }

        fn get_project_versions(
            &self,
            project: &str,
            _loaders: &[String],
            _game_versions: &[String],
        ) -> crate::error::Result<Vec<ProjectVersion>> {
            Ok(self.versions.get(project).cloned().unwrap_or_default())
        }

        fn get_version(&self, version_id: &str) -> crate::error::Result<ProjectVersion> {
            self.versions_by_id
                .get(version_id)
                .cloned()
                .ok_or_else(|| crate::error::MinecliError::message(format!("missing {version_id}")))
        }
    }

    fn config() -> ServerConfig {
        ServerConfig::new(
            "test".to_owned(),
            "1.21.5".to_owned(),
            ServerType::Fabric,
            "world".to_owned(),
        )
    }

    fn project(id: &str) -> Project {
        Project {
            id: id.to_owned(),
            slug: id.to_owned(),
            title: id.to_owned(),
            project_type: "mod".to_owned(),
            server_side: "required".to_owned(),
            client_side: "optional".to_owned(),
        }
    }

    fn version(
        id: &str,
        project_id: &str,
        version_number: &str,
        dependencies: Vec<VersionDependency>,
    ) -> ProjectVersion {
        ProjectVersion {
            id: id.to_owned(),
            project_id: project_id.to_owned(),
            name: format!("{project_id} {version_number}"),
            version_number: version_number.to_owned(),
            version_type: ReleaseChannel::Release,
            game_versions: vec!["1.21.5".to_owned()],
            loaders: vec!["fabric".to_owned()],
            dependencies,
            files: vec![file(&format!("{project_id}.jar"))],
        }
    }

    fn required_dependency(project_id: &str) -> VersionDependency {
        VersionDependency {
            version_id: None,
            project_id: Some(project_id.to_owned()),
            dependency_type: DependencyType::Required,
        }
    }

    fn file(filename: &str) -> ModrinthFile {
        ModrinthFile {
            hashes: BTreeMap::new(),
            url: format!("http://localhost/{filename}"),
            filename: filename.to_owned(),
            primary: true,
            size: 1,
        }
    }

    fn file_from_path(filename: &str, path: &std::path::Path) -> ModrinthFile {
        let bytes = std::fs::read(path).unwrap();
        let mut hasher = Sha512::new();
        hasher.update(&bytes);
        let mut hashes = BTreeMap::new();
        hashes.insert("sha512".to_owned(), hex::encode(hasher.finalize()));

        ModrinthFile {
            hashes,
            url: format!("file://{}", path.display()),
            filename: filename.to_owned(),
            primary: true,
            size: bytes.len() as u64,
        }
    }

    fn slugs(plan: &[PlannedInstall]) -> Vec<&str> {
        plan.iter()
            .map(|item| item.locked_package.slug.as_str())
            .collect()
    }
}

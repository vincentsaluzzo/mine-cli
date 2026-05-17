use std::collections::{BTreeMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command as ProcessCommand;

use reqwest::blocking::Client;
use serde::{Deserialize, Serialize};
use sha1::{Digest as Sha1Digest, Sha1};
use sha2::Sha512;

use crate::cli::{
    BackupsCommand, Command, DatapacksCommand, GlobalOptions, ModpackCommand, ServersCommand,
};
use crate::config::{
    load_global_config, load_server_registry, servers_file, write_global_config,
    write_server_registry,
};
use crate::core::backups::{create_backup_operation, list_backup_operations, rollback_operation};
use crate::core::datapack::{disable_datapack, discover_datapacks, enable_datapack};
use crate::core::history;
use crate::core::lockfile::{LockFile, LockedPackage, load_lockfile, write_lockfile};
use crate::core::manifest::{ServerConfig, minecli_dir, server_file};
use crate::core::manifest::{load_server_config, write_server_config};
use crate::core::modpack::{
    ModrinthPackFile, copy_server_overrides, read_modrinth_pack, validate_relative_path,
};
use crate::core::server::{ContentKind, ServerType, content_kind_from_project_type, detect_server};
use crate::error::{IoResultExt, MinecliError, Result};
use crate::fsops::{cache_dir, copy_verified_download, verify_file_hash};
use crate::sources::modrinth::{
    DependencyType, ModrinthClient, ModrinthFile, ProjectSource, ProjectVersion, ReleaseChannel,
    SearchParams, select_version, version_matches_server,
};
use crate::sources::{SourceId, is_registry_source};

const REGISTRY_SOURCE: &str = "modrinth";

pub fn execute(globals: GlobalOptions, command: Command) -> Result<()> {
    if globals.verbose {
        eprintln!("server path: {}", globals.server_dir.display());
        eprintln!("config dir: {}", globals.config_dir.display());
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
        Command::Search {
            query,
            kind,
            limit,
            all_sides,
        } => search(&globals, query, kind, limit, all_sides),
        Command::Import => import_existing(&globals),
        Command::Export { output } => export(&globals, output),
        Command::Restore { manifest } => restore(&globals, manifest),
        Command::Sync { source } => sync(&globals, source),
        Command::Modpack { command } => modpack(&globals, command),
        Command::Datapacks { command } => datapacks(&globals, command),
        Command::Install {
            project,
            kind,
            file,
            folder,
            version,
            channel,
            no_deps,
        } => install(
            &globals,
            InstallOptions {
                project,
                kind,
                file,
                folder,
                requested_version: version,
                channel,
                no_deps,
            },
        ),
        Command::List { kind, json } => list(&globals, kind, json),
        Command::Outdated { channel, changelog } => outdated(&globals, channel, changelog),
        Command::Update {
            project,
            all,
            channel,
        } => update(&globals, project, all, channel),
        Command::Backups { command } => backups(&globals, command),
        Command::Rollback { operation_id } => rollback(&globals, operation_id),
        Command::Edit { force } => edit(&globals, force),
        Command::Remove {
            project,
            remove_orphans,
        } => remove(&globals, project, remove_orphans),
        Command::Doctor { fix } => doctor(&globals, fix),
        Command::Servers { command } => servers(&globals, command),
    }
}

fn servers(globals: &GlobalOptions, command: ServersCommand) -> Result<()> {
    match command {
        ServersCommand::List => servers_list(globals),
        ServersCommand::Add { name, path } => servers_add(globals, name, path),
        ServersCommand::Remove { name } => servers_remove(globals, name),
        ServersCommand::Show { name } => servers_show(globals, name),
    }
}

fn backups(globals: &GlobalOptions, command: BackupsCommand) -> Result<()> {
    match command {
        BackupsCommand::List => backups_list(globals),
    }
}

fn modpack(globals: &GlobalOptions, command: ModpackCommand) -> Result<()> {
    match command {
        ModpackCommand::Inspect { path } => modpack_inspect(globals, path),
        ModpackCommand::Install { path } => modpack_install(globals, path),
    }
}

fn datapacks(globals: &GlobalOptions, command: DatapacksCommand) -> Result<()> {
    match command {
        DatapacksCommand::List => datapacks_list(globals),
        DatapacksCommand::Disable { datapack } => datapacks_disable(globals, datapack),
        DatapacksCommand::Enable { datapack } => datapacks_enable(globals, datapack),
    }
}

fn datapacks_list(globals: &GlobalOptions) -> Result<()> {
    let config = load_server_config(&globals.server_dir)?;
    let lockfile = load_lockfile(&globals.server_dir)?;
    let entries = discover_datapacks(&globals.server_dir, &config.paths, &lockfile)?;
    if entries.is_empty() {
        println!("No datapacks found.");
        return Ok(());
    }

    println!(
        "{:<24} {:<10} {:<8} {:<8} Path",
        "Name", "State", "Managed", "Format"
    );
    for entry in entries {
        println!(
            "{:<24} {:<10} {:<8} {:<8} {}",
            truncate(&entry.id, 24),
            if entry.enabled { "enabled" } else { "disabled" },
            if entry.managed { "yes" } else { "no" },
            entry
                .pack_format
                .map(|format| format.to_string())
                .unwrap_or_else(|| "-".to_owned()),
            entry.path.display()
        );
        if let Some(description) = entry.description {
            println!("  {description}");
        }
    }
    Ok(())
}

fn datapacks_disable(globals: &GlobalOptions, datapack: String) -> Result<()> {
    let config = load_server_config(&globals.server_dir)?;
    let mut lockfile = load_lockfile(&globals.server_dir)?;
    if globals.dry_run {
        println!("Dry run: would disable datapack `{datapack}`.");
        return Ok(());
    }
    let disabled = disable_datapack(&globals.server_dir, &config.paths, &mut lockfile, &datapack)?;
    write_lockfile(&globals.server_dir, &lockfile)?;
    history::record(
        &globals.server_dir,
        format!("disable datapack {}", disabled.id),
    )?;
    println!(
        "Disabled datapack `{}` -> {}",
        disabled.id,
        disabled.path.display()
    );
    Ok(())
}

fn datapacks_enable(globals: &GlobalOptions, datapack: String) -> Result<()> {
    let config = load_server_config(&globals.server_dir)?;
    let mut lockfile = load_lockfile(&globals.server_dir)?;
    if globals.dry_run {
        println!("Dry run: would enable datapack `{datapack}`.");
        return Ok(());
    }
    let enabled = enable_datapack(&globals.server_dir, &config.paths, &mut lockfile, &datapack)?;
    write_lockfile(&globals.server_dir, &lockfile)?;
    history::record(
        &globals.server_dir,
        format!("enable datapack {}", enabled.id),
    )?;
    println!(
        "Enabled datapack `{}` -> {}",
        enabled.id,
        enabled.path.display()
    );
    Ok(())
}

fn modpack_inspect(globals: &GlobalOptions, path: PathBuf) -> Result<()> {
    let config = load_server_config(&globals.server_dir)?;
    let index = read_modrinth_pack(&path)?;

    println!("Modpack: {}", index.name);
    println!("Version: {}", index.version_id);
    if let Some(summary) = &index.summary {
        println!("Summary: {summary}");
    }
    if let Some(minecraft) = index.dependencies.get("minecraft") {
        println!("Minecraft: {minecraft}");
    }
    if let Some(loader) = index.loader_dependency() {
        println!("Loader: {loader}");
    }

    match index.validate_for_server(config.server_type, &config.minecraft_version) {
        Ok(()) => println!("Server compatibility: compatible"),
        Err(error) => println!("Server compatibility: {error}"),
    }

    println!(
        "Required server files: {}",
        index.required_server_files().len()
    );
    for file in index.required_server_files() {
        println!("  + {}", file.path.display());
    }
    println!(
        "Optional server files: {}",
        index.optional_server_files().len()
    );
    for file in index.optional_server_files() {
        println!("  ? {}", file.path.display());
    }
    Ok(())
}

fn modpack_install(globals: &GlobalOptions, path: PathBuf) -> Result<()> {
    let config = load_server_config(&globals.server_dir)?;
    let mut lockfile = load_lockfile(&globals.server_dir)?;
    let index = read_modrinth_pack(&path)?;
    index.validate_for_server(config.server_type, &config.minecraft_version)?;

    let mut plan = Vec::new();
    for file in index.required_server_files() {
        if let Some(item) = planned_modpack_file(&index.version_id, &index.name, file)? {
            plan.push(item);
        }
    }
    plan.sort_by(|left, right| {
        left.locked_package
            .installed_path
            .cmp(&right.locked_package.installed_path)
    });

    if plan.is_empty() {
        return Err(MinecliError::message(format!(
            "{} has no required server-side package files MineCLI can install",
            index.name
        )));
    }

    print_install_plan(&plan, globals.dry_run);
    let optional = index.optional_server_files();
    if !optional.is_empty() {
        println!("Optional server files skipped:");
        for file in optional {
            println!("  ? {}", file.path.display());
        }
    }
    if globals.dry_run {
        return Ok(());
    }

    validate_install_plan(&globals.server_dir, &lockfile, &plan)?;
    let cache = cache_dir()?;
    let client = reqwest::blocking::Client::new();
    apply_install_plan(&globals.server_dir, &mut lockfile, &client, &cache, plan)?;
    let copied_overrides = copy_server_overrides(&path, &globals.server_dir)?;
    write_lockfile(&globals.server_dir, &lockfile)?;
    history::record(
        &globals.server_dir,
        format!("install modpack {}", index.name),
    )?;
    if copied_overrides > 0 {
        println!("Copied {copied_overrides} override file(s).");
    }
    println!("Modpack install complete.");
    Ok(())
}

fn backups_list(globals: &GlobalOptions) -> Result<()> {
    let operations = list_backup_operations(&globals.server_dir)?;
    if operations.is_empty() {
        println!("No backups found.");
        return Ok(());
    }

    println!("{:<24} {:<12} Files", "Operation", "Created");
    for operation in operations {
        println!(
            "{:<24} {:<12} {}",
            truncate(&operation.id, 24),
            operation.created_at_unix,
            operation.files.len()
        );
        println!("  {}", operation.action);
    }
    Ok(())
}

fn servers_list(globals: &GlobalOptions) -> Result<()> {
    let registry = load_server_registry(&globals.config_dir)?;
    if registry.servers.is_empty() {
        println!("No servers registered.");
        return Ok(());
    }

    println!("{:<20} Path", "Name");
    for (name, server) in registry.servers {
        println!("{:<20} {}", name, server.path.display());
    }
    Ok(())
}

fn servers_add(globals: &GlobalOptions, name: String, path: PathBuf) -> Result<()> {
    if !path.exists() {
        return Err(MinecliError::message(format!(
            "server path does not exist: {}",
            path.display()
        )));
    }
    if !path.is_dir() {
        return Err(MinecliError::message(format!(
            "server path is not a directory: {}",
            path.display()
        )));
    }

    let mut registry = load_server_registry(&globals.config_dir)?;
    registry.add(name.clone(), path.clone())?;
    write_server_registry(&globals.config_dir, &registry)?;

    let mut config = load_global_config(&globals.config_dir)?;
    if config.default_server.is_none() {
        config.default_server = Some(name.clone());
        write_global_config(&globals.config_dir, &config)?;
    }

    println!(
        "Registered server `{name}` at {}",
        path.canonicalize().unwrap_or(path).display()
    );
    println!("Registry: {}", servers_file(&globals.config_dir).display());
    Ok(())
}

fn servers_remove(globals: &GlobalOptions, name: String) -> Result<()> {
    let mut registry = load_server_registry(&globals.config_dir)?;
    let removed = registry.remove(&name)?;
    write_server_registry(&globals.config_dir, &registry)?;

    let mut config = load_global_config(&globals.config_dir)?;
    if config.default_server.as_deref() == Some(&name) {
        config.default_server = None;
        write_global_config(&globals.config_dir, &config)?;
    }

    println!("Removed server `{name}` at {}", removed.path.display());
    Ok(())
}

fn servers_show(globals: &GlobalOptions, name: String) -> Result<()> {
    let registry = load_server_registry(&globals.config_dir)?;
    let server = registry.get(&name)?;
    println!("Name: {name}");
    println!("Path: {}", server.path.display());
    println!("Exists: {}", server.path.exists());
    Ok(())
}

fn init(
    globals: &GlobalOptions,
    server_type: Option<ServerType>,
    minecraft_version: Option<String>,
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

    let detection = detect_server(server_dir)?;
    let detected_type = server_type.unwrap_or(detection.server_type);
    let minecraft_version = minecraft_version
        .or(detection.minecraft_version)
        .ok_or_else(|| {
            MinecliError::message(
                "could not detect Minecraft version; pass --minecraft <version>".to_owned(),
            )
        })?;
    let world = detection.world;
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
    all_sides: bool,
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
        !all_sides,
    );
    let client = ModrinthClient::new()?;
    let response = client.search(&params)?;

    if response.hits.is_empty() {
        println!("No matching packages found.");
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

fn import_existing(globals: &GlobalOptions) -> Result<()> {
    let config = load_server_config(&globals.server_dir)?;
    let mut lockfile = load_lockfile(&globals.server_dir)?;
    let client = ModrinthClient::new()?;
    let plan = plan_import_existing(&client, &globals.server_dir, &config, &lockfile)?;

    println!("Import plan:");
    for package in &plan.matched {
        println!(
            "  + {} {} ({}) -> {}",
            package.slug,
            package.kind,
            package.version_number,
            package.installed_path.display()
        );
    }
    for path in &plan.unmatched {
        println!("  ? unmanaged {}", path.display());
    }

    if globals.dry_run {
        println!("Dry run: no files changed.");
        return Ok(());
    }

    for package in plan.matched {
        lockfile.upsert_package(package);
    }
    write_lockfile(&globals.server_dir, &lockfile)?;
    history::record(&globals.server_dir, "import existing files")?;
    println!("Import complete.");
    Ok(())
}

fn export(globals: &GlobalOptions, output: Option<PathBuf>) -> Result<()> {
    let manifest = export_manifest_from_server(&globals.server_dir)?;
    let contents =
        toml::to_string_pretty(&manifest).map_err(|source| MinecliError::TomlSerialize {
            path: output.clone().unwrap_or_else(|| PathBuf::from("<stdout>")),
            source,
        })?;

    if let Some(output) = output {
        crate::core::manifest::write_atomic(&output, contents.as_bytes())?;
        println!("Exported manifest to {}", output.display());
    } else {
        print!("{contents}");
    }
    Ok(())
}

fn restore(globals: &GlobalOptions, manifest: PathBuf) -> Result<()> {
    let manifest = read_export_manifest(&manifest)?;
    restore_manifest(globals, &manifest, None)
}

fn sync(globals: &GlobalOptions, source: PathBuf) -> Result<()> {
    let manifest = export_manifest_from_server(&source)?;
    restore_manifest(globals, &manifest, Some(&source))
}

struct InstallOptions {
    project: Option<String>,
    kind: Option<ContentKind>,
    file: Option<PathBuf>,
    folder: Option<PathBuf>,
    requested_version: Option<String>,
    channel: ReleaseChannel,
    no_deps: bool,
}

fn install(globals: &GlobalOptions, options: InstallOptions) -> Result<()> {
    match (options.project, options.file, options.folder) {
        (Some(project), None, None) => install_from_registry(
            globals,
            project,
            options.kind,
            options.requested_version,
            options.channel,
            options.no_deps,
        ),
        (None, Some(file), None) => install_local_file(globals, file, options.kind),
        (None, None, Some(folder)) => install_local_folder(globals, folder, options.kind),
        _ => Err(MinecliError::message(
            "install requires exactly one package, --file, or --folder",
        )),
    }
}

fn install_from_registry(
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

fn install_local_file(
    globals: &GlobalOptions,
    source_path: PathBuf,
    kind: Option<ContentKind>,
) -> Result<()> {
    let config = load_server_config(&globals.server_dir)?;
    let mut lockfile = load_lockfile(&globals.server_dir)?;
    let kind = kind.ok_or_else(|| {
        MinecliError::message("install --file requires --kind <mod|plugin|datapack>")
    })?;
    validate_content_kind(config.server_type, kind)?;
    let plan = vec![planned_local_file(
        &config,
        &source_path,
        kind,
        "local-file",
        false,
    )?];

    validate_install_plan(&globals.server_dir, &lockfile, &plan)?;
    print_install_plan(&plan, globals.dry_run);
    if globals.dry_run {
        return Ok(());
    }

    apply_local_install_plan(&globals.server_dir, &mut lockfile, plan)?;
    write_lockfile(&globals.server_dir, &lockfile)?;
    history::record(
        &globals.server_dir,
        format!("install local file {}", source_path.display()),
    )?;
    println!("Install complete.");
    Ok(())
}

fn install_local_folder(
    globals: &GlobalOptions,
    source_dir: PathBuf,
    kind: Option<ContentKind>,
) -> Result<()> {
    let config = load_server_config(&globals.server_dir)?;
    let mut lockfile = load_lockfile(&globals.server_dir)?;
    let kind = kind.ok_or_else(|| {
        MinecliError::message("install --folder requires --kind <mod|plugin|datapack>")
    })?;
    validate_content_kind(config.server_type, kind)?;
    if !source_dir.is_dir() {
        return Err(MinecliError::message(format!(
            "local source folder does not exist: {}",
            source_dir.display()
        )));
    }

    let mut plan = Vec::new();
    for entry in fs::read_dir(&source_dir).at(&source_dir)? {
        let entry = entry.at(&source_dir)?;
        let path = entry.path();
        if !path.is_file() || !is_supported_local_artifact(&path, kind) {
            continue;
        }
        plan.push(planned_local_file(
            &config,
            &path,
            kind,
            "local-folder",
            false,
        )?);
    }
    plan.sort_by(|left, right| left.locked_package.slug.cmp(&right.locked_package.slug));

    if plan.is_empty() {
        println!(
            "No supported package files found in {}.",
            source_dir.display()
        );
        return Ok(());
    }

    validate_install_plan(&globals.server_dir, &lockfile, &plan)?;
    print_install_plan(&plan, globals.dry_run);
    if globals.dry_run {
        return Ok(());
    }

    apply_local_install_plan(&globals.server_dir, &mut lockfile, plan)?;
    write_lockfile(&globals.server_dir, &lockfile)?;
    history::record(
        &globals.server_dir,
        format!("install local folder {}", source_dir.display()),
    )?;
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

fn outdated(globals: &GlobalOptions, channel: ReleaseChannel, show_changelog: bool) -> Result<()> {
    let config = load_server_config(&globals.server_dir)?;
    let lockfile = load_lockfile(&globals.server_dir)?;
    let client = ModrinthClient::new()?;
    let outdated = plan_outdated_packages(&client, &config, &lockfile, channel)?;

    if outdated.is_empty() {
        println!("No outdated registry-backed packages found.");
        return Ok(());
    }

    println!(
        "{:<24} {:<10} {:<16} {:<16} Source",
        "Slug", "Kind", "Installed", "Latest"
    );
    for package in outdated {
        println!(
            "{:<24} {:<10} {:<16} {:<16} {}",
            truncate(&package.slug, 24),
            package.kind,
            truncate(&package.current_version_number, 16),
            truncate(&package.latest_version_number, 16),
            package.source
        );
        if show_changelog {
            match package.changelog_summary.as_deref() {
                Some(summary) => println!("  changelog: {summary}"),
                None => println!("  changelog: unavailable"),
            }
        }
    }

    Ok(())
}

fn update(
    globals: &GlobalOptions,
    project: Option<String>,
    all: bool,
    channel: ReleaseChannel,
) -> Result<()> {
    let config = load_server_config(&globals.server_dir)?;
    let mut lockfile = load_lockfile(&globals.server_dir)?;
    let selected = selected_update_packages(&lockfile, project.as_deref(), all)?;

    if selected.is_empty() {
        println!("No registry-backed packages to update.");
        return Ok(());
    }

    let client = ModrinthClient::new()?;
    let plan = plan_registry_updates(&client, &config, &lockfile, &selected, channel)?;

    if plan.is_empty() {
        println!("All selected packages are already up to date.");
        return Ok(());
    }

    validate_install_plan(&globals.server_dir, &lockfile, &plan)?;
    print_update_plan(&plan, globals.dry_run);
    if globals.dry_run {
        return Ok(());
    }

    let previous_lockfile = lockfile.clone();
    let backup = create_backup_operation(
        &globals.server_dir,
        history_message_for_update(project.as_deref()),
        &packages_replaced_by_plan(&previous_lockfile, &plan),
    )?;
    let cache = cache_dir()?;
    apply_install_plan(
        &globals.server_dir,
        &mut lockfile,
        client.http_client(),
        &cache,
        plan.clone(),
    )?;
    remove_replaced_files(&globals.server_dir, &previous_lockfile, &plan)?;
    write_lockfile(&globals.server_dir, &lockfile)?;

    if let Some(backup) = &backup {
        println!("Backup created: {}", backup.id);
    }
    let history_message = history_message_for_update(project.as_deref());
    history::record(&globals.server_dir, history_message)?;
    println!("Update complete.");
    Ok(())
}

fn rollback(globals: &GlobalOptions, operation_id: String) -> Result<()> {
    let operation = rollback_operation(&globals.server_dir, &operation_id)?;
    history::record(&globals.server_dir, format!("rollback {}", operation.id))?;
    println!(
        "Rollback complete: restored {} file(s) from {}.",
        operation.files.len(),
        operation.id
    );
    Ok(())
}

fn edit(globals: &GlobalOptions, force: bool) -> Result<()> {
    let path = server_file(&globals.server_dir);
    let original = fs::read(&path).at(&path)?;
    let editor = std::env::var("VISUAL")
        .or_else(|_| std::env::var("EDITOR"))
        .map_err(|_| MinecliError::message("set $EDITOR or $VISUAL to use minecli edit"))?;

    let status = ProcessCommand::new(&editor)
        .arg(&path)
        .status()
        .map_err(|source| MinecliError::Io {
            path: PathBuf::from(&editor),
            source,
        })?;
    if !status.success() {
        return Err(MinecliError::message(format!(
            "editor exited with status {status}"
        )));
    }

    match load_server_config(&globals.server_dir).and_then(|config| {
        validate_server_config(&config)?;
        Ok(config)
    }) {
        Ok(_) => {
            history::record(&globals.server_dir, "edit server config")?;
            println!("Config updated: {}", path.display());
            Ok(())
        }
        Err(error) if force => {
            println!("Warning: edited config did not validate: {error}");
            history::record(&globals.server_dir, "edit server config --force")?;
            Ok(())
        }
        Err(error) => {
            crate::core::manifest::write_atomic(&path, &original)?;
            Err(MinecliError::message(format!(
                "edited config is invalid and was restored: {error}"
            )))
        }
    }
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

    let packages_to_backup = to_remove
        .iter()
        .filter_map(|project_id| lockfile.package_by_project_id(project_id).cloned())
        .collect::<Vec<_>>();
    let backup = create_backup_operation(
        &globals.server_dir,
        format!("remove {project}"),
        &packages_to_backup,
    )?;

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
    if let Some(backup) = backup {
        println!("Backup created: {}", backup.id);
    }
    history::record(&globals.server_dir, format!("remove {project}"))?;
    println!("Remove complete.");
    Ok(())
}

fn doctor(globals: &GlobalOptions, fix: bool) -> Result<()> {
    let config = load_server_config(&globals.server_dir)?;
    validate_server_config(&config)?;
    let mut lockfile = load_lockfile(&globals.server_dir)?;
    let mut issues = Vec::new();
    let mut fixes = Vec::new();
    let mut stale_project_ids = Vec::new();

    for path in [
        &config.paths.mods,
        &config.paths.plugins,
        &config.paths.datapacks,
    ] {
        let absolute = globals.server_dir.join(path);
        if !absolute.exists() {
            if fix {
                fs::create_dir_all(&absolute).at(&absolute)?;
                fixes.push(format!("created missing directory: {}", path.display()));
            } else {
                issues.push(format!(
                    "missing directory: {} (run `minecli doctor --fix` to create it)",
                    path.display()
                ));
            }
        }
    }

    for package in &lockfile.packages {
        let path = globals.server_dir.join(&package.installed_path);
        if !path.exists() {
            if fix {
                stale_project_ids.push(package.project_id.clone());
                fixes.push(format!(
                    "removed stale lockfile entry for {}: {}",
                    package.slug,
                    package.installed_path.display()
                ));
            } else {
                issues.push(format!(
                    "stale lockfile entry for {}: missing {} (run `minecli doctor --fix` to remove the entry)",
                    package.slug,
                    package.installed_path.display()
                ));
            }
            continue;
        }

        if let Err(error) = verify_file_hash(&path, &package.hashes, &package.filename) {
            issues.push(format!(
                "{error} (reinstall the package or rollback to a backup)"
            ));
        }
    }

    for project_id in stale_project_ids {
        lockfile.remove_project(&project_id);
    }
    if fix && !fixes.is_empty() {
        write_lockfile(&globals.server_dir, &lockfile)?;
    }

    issues.extend(detect_duplicate_installed_files(
        &globals.server_dir,
        &config,
    )?);
    issues.extend(detect_duplicate_lockfile_entries(&lockfile));
    issues.extend(package_metadata_issues(&lockfile, globals.verbose));

    if !fixes.is_empty() {
        println!("Applied {} fix(es):", fixes.len());
        for fixed in &fixes {
            println!("  - {fixed}");
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

fn validate_server_config(config: &ServerConfig) -> Result<()> {
    if config.name.trim().is_empty() {
        return Err(MinecliError::message("server name cannot be empty"));
    }
    if config.minecraft_version.trim().is_empty() {
        return Err(MinecliError::message("minecraft version cannot be empty"));
    }
    if config.world.trim().is_empty() {
        return Err(MinecliError::message("world name cannot be empty"));
    }
    for (label, path) in [
        ("mods", &config.paths.mods),
        ("plugins", &config.paths.plugins),
        ("datapacks", &config.paths.datapacks),
    ] {
        if path.as_os_str().is_empty() {
            return Err(MinecliError::message(format!(
                "{label} path cannot be empty"
            )));
        }
        if path.is_absolute() {
            return Err(MinecliError::message(format!(
                "{label} path must be relative to the server folder"
            )));
        }
        if path.components().any(|component| {
            matches!(
                component,
                std::path::Component::ParentDir | std::path::Component::RootDir
            )
        }) {
            return Err(MinecliError::message(format!(
                "{label} path cannot escape the server folder"
            )));
        }
    }

    Ok(())
}

fn detect_duplicate_installed_files(
    server_dir: &Path,
    config: &ServerConfig,
) -> Result<Vec<String>> {
    let mut issues = Vec::new();
    for directory in [
        &config.paths.mods,
        &config.paths.plugins,
        &config.paths.datapacks,
    ] {
        let absolute = server_dir.join(directory);
        if !absolute.exists() {
            continue;
        }
        let mut filenames: BTreeMap<String, Vec<PathBuf>> = BTreeMap::new();
        for entry in fs::read_dir(&absolute).at(&absolute)? {
            let entry = entry.at(&absolute)?;
            if !entry.path().is_file() {
                continue;
            }
            let filename = entry.file_name().to_string_lossy().to_lowercase();
            filenames
                .entry(filename)
                .or_default()
                .push(directory.join(entry.file_name()));
        }
        for paths in filenames.values().filter(|paths| paths.len() > 1) {
            issues.push(format!(
                "duplicate installed files with the same case-insensitive name: {}",
                paths
                    .iter()
                    .map(|path| path.display().to_string())
                    .collect::<Vec<_>>()
                    .join(", ")
            ));
        }
    }
    Ok(issues)
}

fn detect_duplicate_lockfile_entries(lockfile: &LockFile) -> Vec<String> {
    let mut issues = Vec::new();
    let mut by_path: BTreeMap<PathBuf, Vec<String>> = BTreeMap::new();
    let mut by_project: BTreeMap<String, Vec<String>> = BTreeMap::new();

    for package in &lockfile.packages {
        by_path
            .entry(package.installed_path.clone())
            .or_default()
            .push(package.slug.clone());
        by_project
            .entry(package.project_id.clone())
            .or_default()
            .push(package.slug.clone());
    }

    for (path, slugs) in by_path.into_iter().filter(|(_, slugs)| slugs.len() > 1) {
        issues.push(format!(
            "duplicate lockfile target {} owned by {}",
            path.display(),
            slugs.join(", ")
        ));
    }
    for (project_id, slugs) in by_project.into_iter().filter(|(_, slugs)| slugs.len() > 1) {
        issues.push(format!(
            "duplicate lockfile project {project_id} listed as {}",
            slugs.join(", ")
        ));
    }

    issues
}

fn package_metadata_issues(lockfile: &LockFile, verbose: bool) -> Vec<String> {
    let mut issues = Vec::new();
    let Ok(client) = ModrinthClient::new() else {
        return issues;
    };
    let mut skipped = 0usize;

    for package in lockfile
        .packages
        .iter()
        .filter(|package| is_registry_source(&package.source))
    {
        if package.source != REGISTRY_SOURCE {
            skipped += 1;
            continue;
        }
        let project = match client.get_project(package.source_project_id_or_project_id()) {
            Ok(project) => project,
            Err(_) => {
                skipped += 1;
                continue;
            }
        };

        if project.server_side == "unsupported" {
            issues.push(format!(
                "{} is marked as unsupported on servers by its package source",
                package.slug
            ));
        }
        if project.client_side == "required" && project.server_side != "required" {
            issues.push(format!(
                "{} is likely client-oriented: client side is required, server side is {}",
                package.slug, project.server_side
            ));
        }
        match content_kind_from_project_type(&project.project_type) {
            Ok(kind) if kind != package.kind => issues.push(format!(
                "{} is tracked as {} but source metadata says {}",
                package.slug, package.kind, kind
            )),
            Ok(_) => {}
            Err(error) => issues.push(format!(
                "{} has incompatible metadata: {error}",
                package.slug
            )),
        }
    }

    if verbose && skipped > 0 {
        eprintln!("skipped source metadata checks for {skipped} package(s)");
    }

    issues
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
                "{} is marked as unsupported on servers by its package source",
                project.slug
            )));
        }
        if project.client_side == "required" && project.server_side != "required" {
            println!(
                "Warning: {} is client-side required and may not be useful on a server.",
                project.slug
            );
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

        let (kind, loader, version) =
            self.select_project_version(&project, requested_kind, requested_version)?;
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
            source: REGISTRY_SOURCE.to_owned(),
            project_id: project.id.clone(),
            source_project_id: Some(project.id.clone()),
            slug: project.slug.clone(),
            title: project.title.clone(),
            kind,
            loader,
            version_id: version.id.clone(),
            source_version_id: Some(version.id.clone()),
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

    fn select_project_version(
        &self,
        project: &crate::sources::modrinth::Project,
        requested_kind: Option<ContentKind>,
        requested_version: Option<&str>,
    ) -> Result<(
        ContentKind,
        Option<String>,
        crate::sources::modrinth::ProjectVersion,
    )> {
        let declared_kind = content_kind_from_project_type(&project.project_type)?;
        let candidates = match requested_kind {
            Some(kind) => vec![kind],
            None => install_kind_candidates(self.config.server_type, declared_kind),
        };

        for kind in &candidates {
            if !self.config.server_type.supports(*kind) {
                continue;
            }
            let loader = self
                .config
                .server_type
                .modrinth_loader(*kind)
                .map(ToOwned::to_owned);
            let versions = self.client.get_project_versions(
                &project.id,
                &loader.iter().cloned().collect::<Vec<_>>(),
                std::slice::from_ref(&self.config.minecraft_version),
            )?;
            if let Some(version) = select_version(&versions, requested_version, self.channel) {
                if *kind != declared_kind && requested_kind.is_none() {
                    println!(
                        "Using {} install for {} because it has a compatible {} version.",
                        kind,
                        project.slug,
                        loader.as_deref().unwrap_or("server")
                    );
                }
                return Ok((*kind, loader, version.clone()));
            }
        }

        if requested_kind.is_some_and(|kind| !self.config.server_type.supports(kind)) {
            return Err(MinecliError::message(format!(
                "{} servers cannot install {} projects",
                self.config.server_type,
                requested_kind.expect("checked")
            )));
        }

        let loaders = candidates
            .iter()
            .filter_map(|kind| self.config.server_type.modrinth_loader(*kind))
            .collect::<Vec<_>>();
        Err(MinecliError::message(format!(
            "no compatible {} version found for Minecraft {}{}",
            project.slug,
            self.config.minecraft_version,
            if loaders.is_empty() {
                String::new()
            } else {
                format!(" and supported loader(s) {}", loaders.join(", "))
            }
        )))
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

fn install_kind_candidates(
    server_type: ServerType,
    declared_kind: ContentKind,
) -> Vec<ContentKind> {
    let mut candidates = Vec::new();
    if server_type.supports(declared_kind) {
        candidates.push(declared_kind);
    }
    for kind in [ContentKind::Plugin, ContentKind::Mod, ContentKind::Datapack] {
        if server_type.supports(kind) && !candidates.contains(&kind) {
            candidates.push(kind);
        }
    }
    if candidates.is_empty() {
        candidates.push(declared_kind);
    }
    candidates
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

fn print_update_plan(plan: &[PlannedInstall], dry_run: bool) {
    println!("Update plan:");
    for item in plan {
        println!(
            "  ~ {} -> {} ({}) -> {} [{} bytes]",
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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct ExportManifest {
    format_version: u32,
    server: ServerConfig,
    #[serde(default)]
    packages: Vec<LockedPackage>,
}

#[derive(Debug, Default)]
struct ImportPlan {
    matched: Vec<LockedPackage>,
    unmatched: Vec<PathBuf>,
}

fn export_manifest_from_server(server_dir: &Path) -> Result<ExportManifest> {
    Ok(ExportManifest {
        format_version: 1,
        server: load_server_config(server_dir)?,
        packages: load_lockfile(server_dir)?.packages,
    })
}

fn read_export_manifest(path: &Path) -> Result<ExportManifest> {
    let contents = fs::read_to_string(path).at(path)?;
    let manifest: ExportManifest =
        toml::from_str(&contents).map_err(|source| MinecliError::TomlDeserialize {
            path: path.to_path_buf(),
            source,
        })?;
    if manifest.format_version != 1 {
        return Err(MinecliError::message(format!(
            "unsupported manifest format version {}",
            manifest.format_version
        )));
    }
    validate_server_config(&manifest.server)?;
    Ok(manifest)
}

fn restore_manifest(
    globals: &GlobalOptions,
    manifest: &ExportManifest,
    sync_source: Option<&Path>,
) -> Result<()> {
    let config_exists = server_file(&globals.server_dir).exists();
    let config = if config_exists {
        let config = load_server_config(&globals.server_dir)?;
        validate_server_config(&config)?;
        config
    } else {
        manifest.server.clone()
    };

    let mut lockfile = load_lockfile(&globals.server_dir)?;
    let client = ModrinthClient::new()?;
    let registry_packages = manifest
        .packages
        .iter()
        .filter(|package| package.source == REGISTRY_SOURCE)
        .cloned()
        .collect::<Vec<_>>();
    let plan = plan_registry_restore(
        &client,
        &config,
        &lockfile,
        &registry_packages,
        ReleaseChannel::Alpha,
    )?;

    println!("Restore plan:");
    for item in &plan {
        println!(
            "  + {} {} ({}) -> {}",
            item.locked_package.slug,
            item.locked_package.kind,
            item.locked_package.version_number,
            item.locked_package.installed_path.display()
        );
    }

    let copied_packages = local_packages_copied_from_sync_source(
        sync_source,
        &globals.server_dir,
        &manifest.packages,
        globals.dry_run,
    )?;
    for package in &copied_packages {
        println!(
            "  + {} {} ({}) -> {}",
            package.slug,
            package.kind,
            package.version_number,
            package.installed_path.display()
        );
    }

    for package in manifest
        .packages
        .iter()
        .filter(|package| package.source != REGISTRY_SOURCE)
    {
        if sync_source.is_none() {
            println!(
                "  ! skipped non-portable {} package {} from {}",
                package.kind, package.slug, package.source
            );
        }
    }

    if globals.dry_run {
        println!("Dry run: no files changed.");
        return Ok(());
    }

    if !config_exists {
        write_server_config(&globals.server_dir, &config)?;
    }
    validate_install_plan(&globals.server_dir, &lockfile, &plan)?;
    let cache = cache_dir()?;
    apply_install_plan(
        &globals.server_dir,
        &mut lockfile,
        client.http_client(),
        &cache,
        plan,
    )?;
    for package in copied_packages {
        lockfile.upsert_package(package);
    }
    write_lockfile(&globals.server_dir, &lockfile)?;
    history::record(&globals.server_dir, "restore manifest")?;
    println!("Restore complete.");
    Ok(())
}

fn plan_registry_restore<S: ProjectSource>(
    source: &S,
    config: &ServerConfig,
    lockfile: &LockFile,
    packages: &[LockedPackage],
    channel: ReleaseChannel,
) -> Result<Vec<PlannedInstall>> {
    let mut planned = Vec::new();
    let mut planned_project_ids = HashSet::new();

    for package in packages {
        let mut resolver = InstallResolver::new(source, config, lockfile, true, channel);
        let package_plan = resolver.resolve(
            package.source_project_id_or_project_id(),
            Some(package.kind),
            Some(&package.version_id),
            package.installed_as_dependency,
        )?;
        for item in package_plan {
            if planned_project_ids.insert(item.locked_package.project_id.clone()) {
                planned.push(item);
            }
        }
    }

    Ok(planned)
}

fn local_packages_copied_from_sync_source(
    sync_source: Option<&Path>,
    target_server_dir: &Path,
    packages: &[LockedPackage],
    dry_run: bool,
) -> Result<Vec<LockedPackage>> {
    let Some(source_server_dir) = sync_source else {
        return Ok(Vec::new());
    };

    let mut copied = Vec::new();
    for package in packages
        .iter()
        .filter(|package| package.source != REGISTRY_SOURCE)
    {
        let source_path = source_server_dir.join(&package.installed_path);
        if !source_path.is_file() {
            println!(
                "  ! skipped {} because source file is missing: {}",
                package.slug,
                package.installed_path.display()
            );
            continue;
        }
        let target_path = target_server_dir.join(&package.installed_path);
        if !dry_run {
            if let Some(parent) = target_path.parent() {
                fs::create_dir_all(parent).at(parent)?;
            }
            fs::copy(&source_path, &target_path).at(&target_path)?;
        }
        copied.push(package.clone());
    }

    Ok(copied)
}

fn plan_import_existing<S: ProjectSource>(
    source: &S,
    server_dir: &Path,
    config: &ServerConfig,
    lockfile: &LockFile,
) -> Result<ImportPlan> {
    let mut plan = ImportPlan::default();
    for (kind, directory) in [
        (ContentKind::Mod, &config.paths.mods),
        (ContentKind::Plugin, &config.paths.plugins),
        (ContentKind::Datapack, &config.paths.datapacks),
    ] {
        if !config.server_type.supports(kind) {
            continue;
        }
        let absolute = server_dir.join(directory);
        if !absolute.exists() {
            continue;
        }

        for entry in fs::read_dir(&absolute).at(&absolute)? {
            let entry = entry.at(&absolute)?;
            let path = entry.path();
            if !path.is_file() || !is_supported_local_artifact(&path, kind) {
                continue;
            }
            let relative = directory.join(entry.file_name());
            if lockfile
                .packages
                .iter()
                .any(|package| package.installed_path == relative)
            {
                continue;
            }

            match import_package_from_file(source, config, &path, relative.clone(), kind)? {
                Some(package) => plan.matched.push(package),
                None => plan.unmatched.push(relative),
            }
        }
    }
    plan.matched
        .sort_by(|left, right| left.slug.cmp(&right.slug));
    plan.unmatched.sort();
    Ok(plan)
}

fn import_package_from_file<S: ProjectSource>(
    source: &S,
    config: &ServerConfig,
    path: &Path,
    installed_path: PathBuf,
    kind: ContentKind,
) -> Result<Option<LockedPackage>> {
    let hashes = local_file_hashes(path)?;
    let Some(sha512) = hashes.get("sha512") else {
        return Ok(None);
    };
    let Some(version) = source.get_version_from_hash(sha512, "sha512")? else {
        return Ok(None);
    };
    let project = source.get_project(&version.project_id)?;
    let filename = installed_path
        .file_name()
        .and_then(|filename| filename.to_str())
        .ok_or_else(|| MinecliError::message("imported file has no valid filename"))?
        .to_owned();
    let dependencies = version
        .dependencies
        .iter()
        .filter(|dependency| dependency.dependency_type == DependencyType::Required)
        .filter_map(|dependency| dependency.project_id.clone())
        .collect::<Vec<_>>();

    Ok(Some(LockedPackage {
        source: REGISTRY_SOURCE.to_owned(),
        project_id: project.id.clone(),
        source_project_id: Some(project.id),
        slug: project.slug.clone(),
        title: project.title,
        kind,
        loader: config
            .server_type
            .modrinth_loader(kind)
            .map(ToOwned::to_owned),
        version_id: version.id.clone(),
        source_version_id: Some(version.id),
        version_number: version.version_number,
        filename,
        hashes,
        installed_path,
        dependencies,
        installed_as_dependency: false,
    }))
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct OutdatedPackage {
    source: String,
    slug: String,
    kind: ContentKind,
    current_version_id: String,
    current_version_number: String,
    latest_version_id: String,
    latest_version_number: String,
    changelog_summary: Option<String>,
}

fn plan_outdated_packages<S: ProjectSource>(
    source: &S,
    config: &ServerConfig,
    lockfile: &LockFile,
    channel: ReleaseChannel,
) -> Result<Vec<OutdatedPackage>> {
    let mut outdated = Vec::new();
    for package in lockfile
        .packages
        .iter()
        .filter(|package| package.source == REGISTRY_SOURCE)
    {
        let Some(latest) = latest_compatible_version(source, config, package, channel)? else {
            continue;
        };
        if latest.id == package.version_id {
            continue;
        }
        outdated.push(OutdatedPackage {
            source: package.source.clone(),
            slug: package.slug.clone(),
            kind: package.kind,
            current_version_id: package.version_id.clone(),
            current_version_number: package.version_number.clone(),
            latest_version_id: latest.id,
            latest_version_number: latest.version_number,
            changelog_summary: latest.changelog.as_deref().and_then(summarize_changelog),
        });
    }

    Ok(outdated)
}

fn summarize_changelog(changelog: &str) -> Option<String> {
    let summary = changelog
        .lines()
        .map(str::trim)
        .find(|line| !line.is_empty())?;
    Some(truncate(summary, 120))
}

fn latest_compatible_version<S: ProjectSource>(
    source: &S,
    config: &ServerConfig,
    package: &LockedPackage,
    channel: ReleaseChannel,
) -> Result<Option<ProjectVersion>> {
    let loader = package.loader.clone().or_else(|| {
        config
            .server_type
            .modrinth_loader(package.kind)
            .map(ToOwned::to_owned)
    });
    let versions = source.get_project_versions(
        package.source_project_id_or_project_id(),
        &loader.into_iter().collect::<Vec<_>>(),
        std::slice::from_ref(&config.minecraft_version),
    )?;

    Ok(select_version(&versions, None, channel).cloned())
}

fn selected_update_packages(
    lockfile: &LockFile,
    project: Option<&str>,
    all: bool,
) -> Result<Vec<LockedPackage>> {
    if all && project.is_some() {
        return Err(MinecliError::message(
            "update accepts either a package or --all, not both",
        ));
    }
    if !all && project.is_none() {
        return Err(MinecliError::message("update requires a package or --all"));
    }

    if all {
        return Ok(lockfile
            .packages
            .iter()
            .filter(|package| package.source == REGISTRY_SOURCE)
            .cloned()
            .collect());
    }

    let project = project.expect("checked above");
    let package = lockfile
        .package_by_query(project)
        .cloned()
        .ok_or_else(|| MinecliError::message(format!("package `{project}` is not installed")))?;
    if package.source != REGISTRY_SOURCE {
        return Err(MinecliError::message(format!(
            "{} was installed from {} and cannot be updated from a registry source",
            package.slug, package.source
        )));
    }

    Ok(vec![package])
}

fn plan_registry_updates<S: ProjectSource>(
    source: &S,
    config: &ServerConfig,
    lockfile: &LockFile,
    selected: &[LockedPackage],
    channel: ReleaseChannel,
) -> Result<Vec<PlannedInstall>> {
    let mut planned = Vec::new();
    let mut planned_project_ids = HashSet::new();

    for package in selected {
        let mut resolver = InstallResolver::new(source, config, lockfile, true, channel);
        let package_plan = resolver.resolve(
            package.source_project_id_or_project_id(),
            Some(package.kind),
            None,
            package.installed_as_dependency,
        )?;
        for item in package_plan {
            if planned_project_ids.insert(item.locked_package.project_id.clone()) {
                planned.push(item);
            }
        }
    }

    Ok(planned)
}

fn packages_replaced_by_plan(lockfile: &LockFile, plan: &[PlannedInstall]) -> Vec<LockedPackage> {
    plan.iter()
        .filter_map(|item| {
            lockfile
                .package_by_project_id(&item.locked_package.project_id)
                .cloned()
        })
        .collect()
}

fn history_message_for_update(project: Option<&str>) -> String {
    match project {
        Some(project) => format!("update {project}"),
        None => "update --all".to_owned(),
    }
}

fn remove_replaced_files(
    server_dir: &Path,
    previous_lockfile: &LockFile,
    plan: &[PlannedInstall],
) -> Result<()> {
    for item in plan {
        let Some(previous) =
            previous_lockfile.package_by_project_id(&item.locked_package.project_id)
        else {
            continue;
        };
        if previous.installed_path == item.locked_package.installed_path {
            continue;
        }

        let previous_path = server_dir.join(&previous.installed_path);
        match fs::remove_file(&previous_path) {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => {
                return Err(MinecliError::Io {
                    path: previous_path,
                    source: error,
                });
            }
        }
    }

    Ok(())
}

fn validate_content_kind(server_type: ServerType, kind: ContentKind) -> Result<()> {
    if server_type.supports(kind) {
        return Ok(());
    }

    Err(MinecliError::message(format!(
        "{} servers cannot install {kind} packages",
        server_type
    )))
}

fn planned_local_file(
    config: &ServerConfig,
    source_path: &Path,
    kind: ContentKind,
    source: &str,
    installed_as_dependency: bool,
) -> Result<PlannedInstall> {
    if !source_path.is_file() {
        return Err(MinecliError::message(format!(
            "local source file does not exist: {}",
            source_path.display()
        )));
    }
    if !is_supported_local_artifact(source_path, kind) {
        return Err(MinecliError::message(format!(
            "unsupported local {kind} file: {}",
            source_path.display()
        )));
    }

    let filename = source_path
        .file_name()
        .and_then(|filename| filename.to_str())
        .ok_or_else(|| MinecliError::message("local source file has no valid filename"))?
        .to_owned();
    let slug = filename
        .rsplit_once('.')
        .map(|(stem, _)| stem)
        .unwrap_or(&filename)
        .to_owned();
    let hashes = local_file_hashes(source_path)?;
    let target_dir = config.paths.target_for(kind);
    let installed_path = target_dir.join(&filename);

    Ok(PlannedInstall {
        version_name: "local".to_owned(),
        locked_package: LockedPackage {
            source: source.to_owned(),
            project_id: format!("local:{}", slug),
            source_project_id: Some(slug.clone()),
            slug: slug.clone(),
            title: slug,
            kind,
            loader: config
                .server_type
                .modrinth_loader(kind)
                .map(ToOwned::to_owned),
            version_id: "local".to_owned(),
            source_version_id: Some("local".to_owned()),
            version_number: "local".to_owned(),
            filename: filename.clone(),
            hashes,
            installed_path: installed_path.clone(),
            dependencies: vec![],
            installed_as_dependency,
        },
        file: ModrinthFile {
            hashes: BTreeMap::new(),
            url: format!("file://{}", source_path.display()),
            filename,
            primary: true,
            size: fs::metadata(source_path).at(source_path)?.len(),
        },
        installed_path,
    })
}

fn planned_modpack_file(
    pack_version_id: &str,
    pack_name: &str,
    file: &ModrinthPackFile,
) -> Result<Option<PlannedInstall>> {
    validate_relative_path(&file.path)?;
    let Some(kind) = file.content_kind() else {
        println!(
            "Skipping unsupported modpack file path: {}",
            file.path.display()
        );
        return Ok(None);
    };
    let Some(url) = file.downloads.first() else {
        return Err(MinecliError::message(format!(
            "modpack file has no download URL: {}",
            file.path.display()
        )));
    };
    let filename = file.filename()?;
    let project_slug = file
        .path
        .file_stem()
        .and_then(|stem| stem.to_str())
        .unwrap_or(&filename)
        .to_owned();
    let source_project_id = format!("{pack_version_id}:{}", file.path.display());
    let source_version_id = file
        .hashes
        .get("sha512")
        .cloned()
        .or_else(|| file.hashes.get("sha1").cloned())
        .unwrap_or_else(|| source_project_id.clone());

    Ok(Some(PlannedInstall {
        version_name: pack_name.to_owned(),
        locked_package: LockedPackage {
            source: SourceId::ModrinthPack.as_str().to_owned(),
            project_id: format!("{}:{source_project_id}", SourceId::ModrinthPack.as_str()),
            source_project_id: Some(source_project_id),
            slug: project_slug.clone(),
            title: project_slug,
            kind,
            loader: None,
            version_id: source_version_id.clone(),
            source_version_id: Some(source_version_id),
            version_number: pack_version_id.to_owned(),
            filename: filename.clone(),
            hashes: file.hashes.clone(),
            installed_path: file.path.clone(),
            dependencies: vec![],
            installed_as_dependency: false,
        },
        file: ModrinthFile {
            hashes: file.hashes.clone(),
            url: url.clone(),
            filename,
            primary: true,
            size: file.file_size,
        },
        installed_path: file.path.clone(),
    }))
}

fn apply_local_install_plan(
    server_dir: &Path,
    lockfile: &mut LockFile,
    plan: Vec<PlannedInstall>,
) -> Result<()> {
    for item in plan {
        let source_path =
            item.file.url.strip_prefix("file://").ok_or_else(|| {
                MinecliError::message("local install plan contained non-file source")
            })?;
        let source_path = Path::new(source_path);
        let target_path = server_dir.join(&item.installed_path);
        if let Some(parent) = target_path.parent() {
            fs::create_dir_all(parent).at(parent)?;
        }
        fs::copy(source_path, &target_path).at(&target_path)?;
        lockfile.upsert_package(item.locked_package);
    }

    Ok(())
}

fn is_supported_local_artifact(path: &Path, kind: ContentKind) -> bool {
    let Some(extension) = path.extension().and_then(|extension| extension.to_str()) else {
        return false;
    };
    match kind {
        ContentKind::Mod | ContentKind::Plugin => extension.eq_ignore_ascii_case("jar"),
        ContentKind::Datapack => {
            extension.eq_ignore_ascii_case("zip") || extension.eq_ignore_ascii_case("jar")
        }
    }
}

fn local_file_hashes(path: &Path) -> Result<BTreeMap<String, String>> {
    let bytes = fs::read(path).at(path)?;
    let mut sha512 = Sha512::new();
    sha512.update(&bytes);
    let mut sha1 = Sha1::new();
    sha1.update(&bytes);

    let mut hashes = BTreeMap::new();
    hashes.insert("sha512".to_owned(), hex::encode(sha512.finalize()));
    hashes.insert("sha1".to_owned(), hex::encode(sha1.finalize()));
    Ok(hashes)
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
    use crate::core::modpack::{ModrinthPackEnv, ModrinthPackFile, SideSupport};
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
            source_project_id: Some(project_id.to_owned()),
            slug: project_id.to_owned(),
            title: project_id.to_owned(),
            kind: ContentKind::Mod,
            loader: Some("fabric".to_owned()),
            version_id: "version".to_owned(),
            source_version_id: Some("version".to_owned()),
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
    fn install_resolver_infers_plugin_kind_when_project_type_is_mod_on_plugin_server() {
        let source = MockSource::new()
            .with_project(project("bluemap"))
            .with_versions(
                "bluemap",
                vec![version("bluemap-paper", "bluemap", "5.20-paper", vec![])],
            );
        let mut config = config();
        config.server_type = ServerType::Purpur;
        let lockfile = LockFile::default();
        let mut resolver =
            InstallResolver::new(&source, &config, &lockfile, true, ReleaseChannel::Release);

        let plan = resolver.resolve("bluemap", None, None, false).unwrap();

        assert_eq!(plan.len(), 1);
        assert_eq!(plan[0].locked_package.kind, ContentKind::Plugin);
        assert_eq!(plan[0].locked_package.loader.as_deref(), Some("paper"));
        assert_eq!(
            plan[0].locked_package.installed_path,
            PathBuf::from("plugins/bluemap.jar")
        );
    }

    #[test]
    fn outdated_planning_detects_registry_packages_and_skips_local_sources() {
        let source = MockSource::new().with_versions(
            "root",
            vec![
                version("root-v2", "root", "2.0.0", vec![]),
                version("root-v1", "root", "1.0.0", vec![]),
            ],
        );
        let config = config();
        let mut root = package("root", vec![], false);
        root.version_id = "root-v1".to_owned();
        root.version_number = "1.0.0".to_owned();
        let mut local = package("local", vec![], false);
        local.source = "local-file".to_owned();
        let lockfile = LockFile {
            packages: vec![root, local],
        };

        let outdated =
            super::plan_outdated_packages(&source, &config, &lockfile, ReleaseChannel::Release)
                .unwrap();

        assert_eq!(outdated.len(), 1);
        assert_eq!(outdated[0].slug, "root");
        assert_eq!(outdated[0].current_version_id, "root-v1");
        assert_eq!(outdated[0].latest_version_id, "root-v2");
        assert_eq!(outdated[0].latest_version_number, "2.0.0");
    }

    #[test]
    fn update_planning_updates_required_dependencies_when_root_is_current() {
        let source = MockSource::new()
            .with_project(project("root"))
            .with_project(project("dep"))
            .with_versions(
                "root",
                vec![version(
                    "root-v1",
                    "root",
                    "1.0.0",
                    vec![required_dependency("dep")],
                )],
            )
            .with_versions(
                "dep",
                vec![
                    version("dep-v2", "dep", "2.0.0", vec![]),
                    version("dep-v1", "dep", "1.0.0", vec![]),
                ],
            );
        let config = config();
        let mut root = package("root", vec!["dep".to_owned()], false);
        root.version_id = "root-v1".to_owned();
        let mut dep = package("dep", vec![], true);
        dep.version_id = "dep-v1".to_owned();
        let lockfile = LockFile {
            packages: vec![root.clone(), dep],
        };

        let plan = super::plan_registry_updates(
            &source,
            &config,
            &lockfile,
            &[root],
            ReleaseChannel::Release,
        )
        .unwrap();

        assert_eq!(slugs(&plan), vec!["dep"]);
        assert_eq!(plan[0].locked_package.version_id, "dep-v2");
        assert!(plan[0].locked_package.installed_as_dependency);
    }

    #[test]
    fn selected_update_packages_rejects_local_sources() {
        let mut local = package("local", vec![], false);
        local.source = "local-file".to_owned();
        let lockfile = LockFile {
            packages: vec![local],
        };

        let result = super::selected_update_packages(&lockfile, Some("local"), false);

        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("local-file"));
    }

    #[test]
    fn import_planning_matches_existing_file_by_sha512_hash() {
        let temp = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(temp.path().join("mods")).unwrap();
        let file_path = temp.path().join("mods/root.jar");
        std::fs::write(&file_path, b"root-jar").unwrap();
        let hashes = super::local_file_hashes(&file_path).unwrap();
        let source = MockSource::new().with_project(project("root")).with_hash(
            hashes.get("sha512").unwrap(),
            version("root-version", "root", "1.0.0", vec![]),
        );
        let config = config();

        let plan = super::plan_import_existing(&source, temp.path(), &config, &LockFile::default())
            .unwrap();

        assert_eq!(plan.matched.len(), 1);
        assert_eq!(plan.matched[0].slug, "root");
        assert_eq!(
            plan.matched[0].installed_path,
            PathBuf::from("mods/root.jar")
        );
        assert!(plan.unmatched.is_empty());
    }

    #[test]
    fn modpack_planning_tracks_required_server_file_as_modrinth_pack_source() {
        let file = ModrinthPackFile {
            path: PathBuf::from("mods/server.jar"),
            hashes: BTreeMap::from([("sha512".to_owned(), "abc".to_owned())]),
            env: Some(ModrinthPackEnv {
                client: Some(SideSupport::Required),
                server: Some(SideSupport::Required),
            }),
            downloads: vec!["file:///tmp/server.jar".to_owned()],
            file_size: 123,
        };

        let planned = super::planned_modpack_file("pack-version", "Test Pack", &file)
            .unwrap()
            .unwrap();

        assert_eq!(planned.locked_package.source, "modrinth-pack");
        assert_eq!(planned.locked_package.kind, ContentKind::Mod);
        assert_eq!(
            planned.locked_package.installed_path,
            PathBuf::from("mods/server.jar")
        );
        assert_eq!(planned.file.url, "file:///tmp/server.jar");
    }

    #[test]
    fn export_manifest_contains_server_config_and_lockfile_packages() {
        let temp = tempfile::tempdir().unwrap();
        let config = config();
        let package = package("root", vec![], false);
        crate::core::manifest::write_server_config(temp.path(), &config).unwrap();
        crate::core::lockfile::write_lockfile(
            temp.path(),
            &LockFile {
                packages: vec![package.clone()],
            },
        )
        .unwrap();

        let manifest = super::export_manifest_from_server(temp.path()).unwrap();

        assert_eq!(manifest.format_version, 1);
        assert_eq!(manifest.server, config);
        assert_eq!(manifest.packages, vec![package]);
    }

    #[test]
    fn server_config_validation_rejects_unsafe_paths() {
        let mut config = config();
        config.paths.mods = PathBuf::from("/tmp/mods");

        let result = super::validate_server_config(&config);

        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("relative"));
    }

    #[test]
    fn diagnostics_detect_duplicate_lockfile_entries() {
        let lockfile = LockFile {
            packages: vec![
                package("root", vec![], false),
                package("root", vec![], false),
            ],
        };

        let issues = super::detect_duplicate_lockfile_entries(&lockfile);

        assert!(issues.iter().any(|issue| issue.contains("duplicate")));
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
        versions_by_hash: HashMap<String, ProjectVersion>,
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

        fn with_hash(mut self, hash: &str, version: ProjectVersion) -> Self {
            self.versions_by_hash.insert(hash.to_owned(), version);
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

        fn get_version_from_hash(
            &self,
            hash: &str,
            _algorithm: &str,
        ) -> crate::error::Result<Option<ProjectVersion>> {
            Ok(self.versions_by_hash.get(hash).cloned())
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
            changelog: None,
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

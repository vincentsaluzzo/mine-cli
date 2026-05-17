use std::path::PathBuf;

use clap::{Parser, Subcommand};

use crate::commands;
use crate::core::server::{ContentKind, ServerType};
use crate::error::Result;
use crate::sources::modrinth::ReleaseChannel;

#[derive(Debug, Parser)]
#[command(name = "minecli")]
#[command(version)]
#[command(about = "Manage Minecraft server mods, datapacks, and plugins.")]
pub struct Cli {
    #[arg(
        long,
        global = true,
        value_name = "PATH",
        help = "Minecraft server folder"
    )]
    pub path: Option<PathBuf>,

    #[arg(
        long,
        global = true,
        value_name = "PATH",
        help = "MineCLI config directory"
    )]
    pub config: Option<PathBuf>,

    #[arg(
        long,
        global = true,
        value_name = "NAME",
        help = "Registered server name"
    )]
    pub server: Option<String>,

    #[arg(
        long,
        global = true,
        help = "Print the planned changes without writing files"
    )]
    pub dry_run: bool,

    #[arg(
        long,
        global = true,
        help = "Reserved flag for non-interactive confirmations"
    )]
    pub yes: bool,

    #[arg(short, long, global = true, help = "Print extra diagnostic output")]
    pub verbose: bool,

    #[command(subcommand)]
    pub command: Command,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    /// Initialize MineCLI state in a Minecraft server folder.
    Init {
        #[arg(long = "type", value_enum, help = "Minecraft server type")]
        server_type: Option<ServerType>,

        #[arg(long, help = "Minecraft version used by this server")]
        minecraft: Option<String>,

        #[arg(long, help = "Friendly server name")]
        name: Option<String>,

        #[arg(long, help = "Overwrite existing .minecli state")]
        force: bool,
    },
    /// Show local server state and package counts.
    Status,
    /// Search available packages.
    Search {
        query: String,

        #[arg(long, value_enum, help = "Limit search to a content kind")]
        kind: Option<ContentKind>,

        #[arg(long, default_value_t = 10, help = "Maximum number of results")]
        limit: usize,

        #[arg(long, help = "Include packages not marked server-side compatible")]
        all_sides: bool,
    },
    /// Import existing files from mods, plugins, and datapack folders.
    Import,
    /// Export a portable MineCLI manifest.
    Export {
        #[arg(short, long, value_name = "PATH", help = "Write manifest to a file")]
        output: Option<PathBuf>,
    },
    /// Restore registry-backed packages from an exported manifest.
    Restore {
        #[arg(value_name = "MANIFEST")]
        manifest: PathBuf,
    },
    /// Sync another server's MineCLI manifest into this server.
    Sync {
        #[arg(value_name = "SOURCE_SERVER")]
        source: PathBuf,
    },
    /// Inspect or install Modrinth modpack files.
    Modpack {
        #[command(subcommand)]
        command: ModpackCommand,
    },
    /// Manage datapacks in the configured world.
    Datapacks {
        #[command(subcommand)]
        command: DatapacksCommand,
    },
    /// Install a package into the server folder.
    Install {
        project: Option<String>,

        #[arg(long, value_enum, help = "Expected project content kind")]
        kind: Option<ContentKind>,

        #[arg(long, value_name = "PATH", help = "Install one local package file")]
        file: Option<PathBuf>,

        #[arg(
            long,
            value_name = "PATH",
            help = "Install all package files from a local folder"
        )]
        folder: Option<PathBuf>,

        #[arg(long, help = "Version ID or version number to install")]
        version: Option<String>,

        #[arg(long, value_enum, default_value_t = ReleaseChannel::Release, help = "Allowed release channel")]
        channel: ReleaseChannel,

        #[arg(long, help = "Do not install required dependencies")]
        no_deps: bool,
    },
    /// List packages tracked in the local lockfile.
    List {
        #[arg(long, value_enum, help = "Limit output to a content kind")]
        kind: Option<ContentKind>,

        #[arg(long, help = "Print machine-readable JSON")]
        json: bool,
    },
    /// Show registry-backed packages with newer compatible versions.
    Outdated {
        #[arg(long, value_enum, default_value_t = ReleaseChannel::Release, help = "Allowed release channel")]
        channel: ReleaseChannel,

        #[arg(long, help = "Show latest-version changelog summaries when available")]
        changelog: bool,
    },
    /// Update registry-backed packages to newer compatible versions.
    Update {
        project: Option<String>,

        #[arg(long, help = "Update all registry-backed packages")]
        all: bool,

        #[arg(long, value_enum, default_value_t = ReleaseChannel::Release, help = "Allowed release channel")]
        channel: ReleaseChannel,
    },
    /// Manage MineCLI file backups.
    Backups {
        #[command(subcommand)]
        command: BackupsCommand,
    },
    /// Restore files and lockfile entries from a backup operation.
    Rollback {
        #[arg(value_name = "OPERATION_ID")]
        operation_id: String,
    },
    /// Edit the local server config in $EDITOR.
    Edit {
        #[arg(long, help = "Keep edited config even when validation fails")]
        force: bool,
    },
    /// Remove a package tracked by MineCLI.
    Remove {
        project: String,

        #[arg(long, help = "Also remove dependencies no remaining package needs")]
        remove_orphans: bool,
    },
    /// Check local folders, lockfile entries, and hashes.
    Doctor {
        #[arg(long, help = "Apply safe automatic fixes")]
        fix: bool,
    },
    /// Manage globally registered server folders.
    Servers {
        #[command(subcommand)]
        command: ServersCommand,
    },
}

#[derive(Debug, Subcommand)]
pub enum ServersCommand {
    /// List registered servers.
    List,
    /// Register a server folder.
    Add { name: String, path: PathBuf },
    /// Remove a registered server folder.
    Remove { name: String },
    /// Show one registered server folder.
    Show { name: String },
}

#[derive(Debug, Subcommand)]
pub enum BackupsCommand {
    /// List backup operations for this server.
    List,
}

#[derive(Debug, Subcommand)]
pub enum ModpackCommand {
    /// Show server-side content from a .mrpack file.
    Inspect {
        #[arg(value_name = "MRPACK")]
        path: PathBuf,
    },
    /// Install required server-side files from a .mrpack file.
    Install {
        #[arg(value_name = "MRPACK")]
        path: PathBuf,
    },
}

#[derive(Debug, Subcommand)]
pub enum DatapacksCommand {
    /// List enabled and disabled datapacks.
    List,
    /// Move a datapack out of the world datapacks folder.
    Disable { datapack: String },
    /// Move a disabled datapack back into the world datapacks folder.
    Enable { datapack: String },
}

#[derive(Debug, Clone)]
pub struct GlobalOptions {
    pub server_dir: PathBuf,
    pub config_dir: PathBuf,
    pub dry_run: bool,
    pub yes: bool,
    pub verbose: bool,
}

pub fn run() -> Result<()> {
    let cli = Cli::parse();
    let config_dir = crate::config::config_dir(cli.config.as_deref())?;
    let server_dir = resolve_server_dir(cli.path, cli.server.as_deref(), &config_dir)?;
    let globals = GlobalOptions {
        server_dir,
        config_dir,
        dry_run: cli.dry_run,
        yes: cli.yes,
        verbose: cli.verbose,
    };

    commands::execute(globals, cli.command)
}

fn resolve_server_dir(
    path: Option<PathBuf>,
    server: Option<&str>,
    config_dir: &std::path::Path,
) -> Result<PathBuf> {
    if let Some(path) = path {
        return Ok(path);
    }

    if let Some(server) = server {
        let registry = crate::config::load_server_registry(config_dir)?;
        return Ok(registry.get(server)?.path.clone());
    }

    std::env::current_dir().map_err(|source| crate::error::MinecliError::Io {
        path: PathBuf::from("."),
        source,
    })
}

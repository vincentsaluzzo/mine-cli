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
        help = "Reserved config file override"
    )]
    pub config: Option<PathBuf>,

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
        minecraft: String,

        #[arg(long, help = "Friendly server name")]
        name: Option<String>,

        #[arg(long, help = "Overwrite existing .minecli state")]
        force: bool,
    },
    /// Show local server state and package counts.
    Status,
    /// Search Modrinth for server-side projects.
    Search {
        query: String,

        #[arg(long, value_enum, help = "Limit search to a content kind")]
        kind: Option<ContentKind>,

        #[arg(long, default_value_t = 10, help = "Maximum number of results")]
        limit: usize,
    },
    /// Install a Modrinth project into the server folder.
    Install {
        project: String,

        #[arg(long, value_enum, help = "Expected project content kind")]
        kind: Option<ContentKind>,

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
    /// Remove a package tracked by MineCLI.
    Remove {
        project: String,

        #[arg(long, help = "Also remove dependencies no remaining package needs")]
        remove_orphans: bool,
    },
    /// Check local folders, lockfile entries, and hashes.
    Doctor,
}

#[derive(Debug, Clone)]
pub struct GlobalOptions {
    pub server_dir: PathBuf,
    pub config: Option<PathBuf>,
    pub dry_run: bool,
    pub yes: bool,
    pub verbose: bool,
}

pub fn run() -> Result<()> {
    let cli = Cli::parse();
    let server_dir = match cli.path {
        Some(path) => path,
        None => std::env::current_dir().map_err(|source| crate::error::MinecliError::Io {
            path: PathBuf::from("."),
            source,
        })?,
    };
    let globals = GlobalOptions {
        server_dir,
        config: cli.config,
        dry_run: cli.dry_run,
        yes: cli.yes,
        verbose: cli.verbose,
    };

    commands::execute(globals, cli.command)
}

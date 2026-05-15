use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::core::manifest::minecli_dir;
use crate::error::{IoResultExt, Result};

pub const HISTORY_FILE: &str = "history.log";

pub fn history_file(server_dir: &Path) -> PathBuf {
    minecli_dir(server_dir).join(HISTORY_FILE)
}

pub fn record(server_dir: &Path, message: impl AsRef<str>) -> Result<()> {
    let path = history_file(server_dir);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).at(parent)?;
    }

    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
        .at(&path)?;
    writeln!(file, "{timestamp} {}", message.as_ref()).at(&path)?;
    Ok(())
}

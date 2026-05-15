use std::collections::BTreeMap;
use std::fs;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

use directories::ProjectDirs;
use reqwest::blocking::Client;
use sha1::{Digest as Sha1Digest, Sha1};
use sha2::Sha512;

use crate::error::{IoResultExt, MinecliError, Result};
use crate::sources::modrinth::ModrinthFile;

pub fn cache_dir() -> Result<PathBuf> {
    if let Some(path) = std::env::var_os("MINECLI_CACHE_DIR") {
        return Ok(PathBuf::from(path).join("downloads"));
    }

    if let Some(project_dirs) = ProjectDirs::from("", "", "minecli") {
        Ok(project_dirs.cache_dir().join("downloads"))
    } else {
        std::env::current_dir()
            .map(|cwd| cwd.join(".minecli").join("cache").join("downloads"))
            .map_err(|source| MinecliError::Io {
                path: PathBuf::from("."),
                source,
            })
    }
}

pub fn copy_verified_download(
    client: &Client,
    file: &ModrinthFile,
    cache_dir: &Path,
) -> Result<PathBuf> {
    fs::create_dir_all(cache_dir).at(cache_dir)?;
    let cache_name = cache_filename(file);
    let cache_path = cache_dir.join(cache_name);

    if cache_path.exists() {
        verify_file_hash(&cache_path, &file.hashes, &file.filename)?;
        return Ok(cache_path);
    }

    let temp_path = cache_dir.join(format!(".{}.download", file.filename));
    let mut output = fs::File::create(&temp_path).at(&temp_path)?;
    if let Some(local_path) = file.url.strip_prefix("file://") {
        let mut input = fs::File::open(local_path).at(local_path)?;
        std::io::copy(&mut input, &mut output).at(&temp_path)?;
    } else {
        let mut response = client.get(&file.url).send()?.error_for_status()?;
        response.copy_to(&mut output)?;
    }
    output.flush().at(&temp_path)?;

    verify_file_hash(&temp_path, &file.hashes, &file.filename)?;
    fs::rename(&temp_path, &cache_path).at(&cache_path)?;
    Ok(cache_path)
}

pub fn verify_file_hash(
    path: &Path,
    hashes: &BTreeMap<String, String>,
    filename: &str,
) -> Result<()> {
    if let Some(expected) = hashes.get("sha512") {
        let actual = hash_sha512(path)?;
        if !actual.eq_ignore_ascii_case(expected) {
            return Err(MinecliError::HashMismatch {
                filename: filename.to_owned(),
                expected: expected.to_owned(),
                actual,
            });
        }
    } else if let Some(expected) = hashes.get("sha1") {
        let actual = hash_sha1(path)?;
        if !actual.eq_ignore_ascii_case(expected) {
            return Err(MinecliError::HashMismatch {
                filename: filename.to_owned(),
                expected: expected.to_owned(),
                actual,
            });
        }
    }

    Ok(())
}

pub fn hash_sha512(path: &Path) -> Result<String> {
    let mut file = fs::File::open(path).at(path)?;
    let mut hasher = Sha512::new();
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        let read = file.read(&mut buffer).at(path)?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    Ok(hex::encode(hasher.finalize()))
}

pub fn hash_sha1(path: &Path) -> Result<String> {
    let mut file = fs::File::open(path).at(path)?;
    let mut hasher = Sha1::new();
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        let read = file.read(&mut buffer).at(path)?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    Ok(hex::encode(hasher.finalize()))
}

fn cache_filename(file: &ModrinthFile) -> String {
    if let Some(hash) = file.hashes.get("sha512") {
        format!("sha512-{hash}-{}", file.filename)
    } else if let Some(hash) = file.hashes.get("sha1") {
        format!("sha1-{hash}-{}", file.filename)
    } else {
        file.filename.clone()
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use crate::fsops::{hash_sha1, hash_sha512, verify_file_hash};

    #[test]
    fn verifies_sha512_hashes() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("file.jar");
        std::fs::write(&path, b"minecli").unwrap();
        let hash = hash_sha512(&path).unwrap();
        let mut hashes = BTreeMap::new();
        hashes.insert("sha512".to_owned(), hash);

        verify_file_hash(&path, &hashes, "file.jar").unwrap();
    }

    #[test]
    fn verifies_sha1_hashes_as_fallback() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("file.jar");
        std::fs::write(&path, b"minecli").unwrap();
        let hash = hash_sha1(&path).unwrap();
        let mut hashes = BTreeMap::new();
        hashes.insert("sha1".to_owned(), hash);

        verify_file_hash(&path, &hashes, "file.jar").unwrap();
    }
}

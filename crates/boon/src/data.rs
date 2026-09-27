//! Verified boon-data installations shared with the Python CLI.
//!
//! Name lookups prefer the newest verified local version. When none exists,
//! they download the latest release into `~/.boon` (or `BOON_DATA_DIR`).

use std::{
    collections::BTreeMap,
    fs,
    io::Read,
    path::{Path, PathBuf},
};

#[cfg(not(target_family = "wasm"))]
use std::time::Duration;

use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};

const INDEX_URL: &str =
    "https://raw.githubusercontent.com/pnxenopoulos/boon-data/data-index/versions.json";
const DOWNLOAD_URL: &str = "https://github.com/pnxenopoulos/boon-data/releases/download";
const SOURCE_REPO: &str = "SteamTracking/GameTracking-Deadlock";
const FILES: [&str; 5] = [
    "abilities.json",
    "heroes.json",
    "manifest.json",
    "misc.json",
    "modifiers.json",
];
const MAX_METADATA: u64 = 16 * 1024 * 1024;
const MAX_FILE: u64 = 128 * 1024 * 1024;

/// Errors while acquiring or reading boon-data catalogs.
#[derive(Debug, thiserror::Error)]
pub enum DataError {
    #[error("boon-data IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("invalid boon-data JSON: {0}")]
    Json(#[from] serde_json::Error),
    #[error(
        "could not download boon-data: {0}; use `boon versions` and `boon get VERSION` to install catalogs"
    )]
    Http(#[from] reqwest::Error),
    #[error("{0}")]
    Invalid(String),
}

pub type Result<T> = std::result::Result<T, DataError>;

#[derive(Debug, Deserialize, Serialize, PartialEq, Eq)]
struct Artifact {
    bytes: u64,
    sha256: String,
    url: String,
}

#[derive(Debug, Deserialize, Serialize)]
struct Entry {
    client_version: String,
    snapshot: String,
    artifacts: BTreeMap<String, Artifact>,
    #[serde(flatten)]
    metadata: BTreeMap<String, Value>,
}

#[derive(Deserialize, Serialize)]
struct Receipt {
    version: Entry,
}

#[derive(Deserialize)]
struct Index {
    source_repository: String,
    latest: Option<String>,
    versions: BTreeMap<String, Entry>,
    snapshots: BTreeMap<String, Snapshot>,
}

#[derive(Deserialize)]
struct Snapshot {
    artifacts: BTreeMap<String, Artifact>,
    released_at: String,
}

fn invalid(message: impl Into<String>) -> DataError {
    DataError::Invalid(message.into())
}

fn valid_version(version: &str) -> bool {
    !version.is_empty() && version.bytes().all(|b| b.is_ascii_digit())
}

fn validate_entry(entry: &Entry, version: &str) -> Result<()> {
    if !valid_version(version) || entry.client_version != version || !valid_version(&entry.snapshot)
    {
        return Err(invalid(
            "invalid client version or snapshot in boon-data metadata",
        ));
    }
    if entry.artifacts.len() != FILES.len()
        || FILES
            .iter()
            .any(|name| !entry.artifacts.contains_key(*name))
    {
        return Err(invalid(
            "boon-data release must contain all five JSON files",
        ));
    }
    for (name, artifact) in &entry.artifacts {
        if artifact.bytes > MAX_FILE
            || artifact.sha256.len() != 64
            || !artifact
                .sha256
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
            || artifact.url != format!("{DOWNLOAD_URL}/{}/{name}", entry.snapshot)
        {
            return Err(invalid(format!("invalid artifact metadata for {name}")));
        }
    }
    Ok(())
}

fn read_file(path: &Path, limit: u64) -> Result<Vec<u8>> {
    let metadata = fs::symlink_metadata(path)?;
    if !metadata.is_file() || metadata.len() > limit {
        return Err(invalid(format!("invalid catalog file: {}", path.display())));
    }
    let mut bytes = Vec::new();
    fs::File::open(path)?
        .take(limit + 1)
        .read_to_end(&mut bytes)?;
    if bytes.len() as u64 > limit {
        return Err(invalid(format!(
            "catalog file exceeds size limit: {}",
            path.display()
        )));
    }
    Ok(bytes)
}

fn verify_bytes(bytes: &[u8], artifact: &Artifact) -> Result<()> {
    if bytes.len() as u64 != artifact.bytes
        || format!("{:x}", Sha256::digest(bytes)) != artifact.sha256
    {
        return Err(invalid("boon-data artifact size or SHA-256 does not match"));
    }
    Ok(())
}

fn verify_files(directory: &Path, entry: &Entry) -> Result<()> {
    for (name, artifact) in &entry.artifacts {
        verify_bytes(&read_file(&directory.join(name), MAX_FILE)?, artifact)?;
    }
    let manifest: Value =
        serde_json::from_slice(&read_file(&directory.join("manifest.json"), MAX_METADATA)?)?;
    let commit = manifest["source"]["commit"].as_str().unwrap_or_default();
    if manifest["release_key"] != entry.snapshot
        || manifest["client_version"] != entry.snapshot
        || manifest["source"]["repository"] != SOURCE_REPO
        || commit.len() != 40
        || !commit.bytes().all(|b| b.is_ascii_hexdigit())
        || manifest["artifacts"].as_object().map(|a| a.len()) != Some(FILES.len() - 1)
    {
        return Err(invalid("invalid boon-data release manifest"));
    }
    for (name, artifact) in &entry.artifacts {
        if name != "manifest.json"
            && (manifest["artifacts"][name]["bytes"] != artifact.bytes
                || manifest["artifacts"][name]["sha256"] != artifact.sha256)
        {
            return Err(invalid(format!("manifest and index disagree for {name}")));
        }
    }
    Ok(())
}

fn installed(directory: &Path, version: &str) -> Result<Entry> {
    if fs::symlink_metadata(directory)?.file_type().is_symlink() {
        return Err(invalid("boon-data version directory must not be a symlink"));
    }
    let receipt: Receipt =
        serde_json::from_slice(&read_file(&directory.join(".install.json"), MAX_METADATA)?)?;
    validate_entry(&receipt.version, version)?;
    verify_files(directory, &receipt.version)?;
    Ok(receipt.version)
}

/// The shared cache root. `BOON_DATA_DIR` overrides the default `~/.boon`.
pub fn cache_dir() -> Result<PathBuf> {
    if let Some(path) = std::env::var_os("BOON_DATA_DIR") {
        let path = PathBuf::from(path);
        if let Ok(suffix) = path.strip_prefix("~") {
            return std::env::home_dir()
                .map(|home| home.join(suffix))
                .ok_or_else(|| invalid("home directory is unavailable; set BOON_DATA_DIR"));
        }
        return Ok(path);
    }
    std::env::home_dir()
        .map(|home| home.join(".boon"))
        .ok_or_else(|| invalid("home directory is unavailable; set BOON_DATA_DIR"))
}

/// Find a verified installation, or download the requested/latest release.
///
/// An explicit version is never replaced with another version. Existing corrupt
/// installations require `boon get VERSION --force` to repair them.
pub fn catalog_dir(version: Option<&str>) -> Result<PathBuf> {
    let root = cache_dir()?;
    #[cfg(not(target_family = "wasm"))]
    let mut client = None;
    resolve(&root, version, &mut |url, limit| {
        #[cfg(not(target_family = "wasm"))]
        {
            if client.is_none() {
                client = Some(
                    reqwest::blocking::Client::builder()
                        .user_agent("boon")
                        .timeout(Duration::from_secs(60))
                        .build()?,
                );
            }
            let client = client
                .as_ref()
                .ok_or_else(|| invalid("HTTP client is unavailable"))?;
            let mut bytes = Vec::new();
            client
                .get(url)
                .send()?
                .error_for_status()?
                .take(limit + 1)
                .read_to_end(&mut bytes)?;
            if bytes.len() as u64 > limit {
                return Err(invalid("boon-data download exceeds size limit"));
            }
            Ok(bytes)
        }
        #[cfg(target_family = "wasm")]
        {
            let _ = (url, limit);
            Err(invalid("boon-data downloads are unavailable on WebAssembly"))
        }
    })
}

fn resolve(
    root: &Path,
    version: Option<&str>,
    fetch: &mut impl FnMut(&str, u64) -> Result<Vec<u8>>,
) -> Result<PathBuf> {
    if let Some(version) = version {
        if !valid_version(version) {
            return Err(invalid(
                "expected a numeric client version from `boon versions`",
            ));
        }
        let directory = root.join(version);
        if directory.exists() || directory.is_symlink() {
            installed(&directory, version)
                .map_err(|error| invalid(format!("{error}; run `boon get {version} --force`")))?;
            return Ok(directory);
        }
    } else if root.exists() {
        let mut versions = fs::read_dir(root)?.collect::<std::io::Result<Vec<_>>>()?;
        // Numeric strings sort by significant length and then lexicographically.
        versions.sort_by_key(|entry| {
            let name = entry
                .file_name()
                .to_string_lossy()
                .trim_start_matches('0')
                .to_owned();
            (name.len(), name)
        });
        for entry in versions.into_iter().rev() {
            let name = entry.file_name();
            let version = name.to_string_lossy();
            if valid_version(&version) && installed(&entry.path(), &version).is_ok() {
                return Ok(entry.path());
            }
        }
    }
    let index: Index = serde_json::from_slice(&fetch(INDEX_URL, MAX_METADATA)?)?;
    if index.source_repository != SOURCE_REPO {
        return Err(invalid("unexpected boon-data source repository"));
    }
    let version = version
        .or(index.latest.as_deref())
        .or_else(|| {
            index
                .versions
                .keys()
                .max_by_key(|version| {
                    let significant = version.trim_start_matches('0');
                    (significant.len(), significant)
                })
                .map(String::as_str)
        })
        .ok_or_else(|| invalid("no boon-data versions have been published"))?;
    let entry = index.versions.get(version).ok_or_else(|| {
        invalid(format!(
            "client version {version} is not available; check `boon versions`"
        ))
    })?;
    validate_entry(entry, version)?;
    let snapshot = index
        .snapshots
        .get(&entry.snapshot)
        .ok_or_else(|| invalid("snapshot is missing from boon-data index"))?;
    if snapshot.artifacts != entry.artifacts
        || entry.metadata.get("released_at").and_then(Value::as_str)
            != Some(snapshot.released_at.as_str())
    {
        return Err(invalid("inconsistent boon-data release metadata"));
    }
    let destination = root.join(version);
    fs::create_dir_all(root)?;
    let temporary = tempfile::Builder::new()
        .prefix(".download-")
        .tempdir_in(root)?;
    let stage = temporary.path().join("snapshot");
    fs::create_dir(&stage)?;
    for (name, artifact) in &entry.artifacts {
        let bytes = fetch(&artifact.url, artifact.bytes)?;
        verify_bytes(&bytes, artifact)?;
        fs::write(stage.join(name), bytes)?;
    }
    verify_files(&stage, entry)?;
    fs::write(
        stage.join(".install.json"),
        serde_json::to_vec(&serde_json::json!({"version": entry}))?,
    )?;
    if destination.exists() || destination.is_symlink() {
        let existing = installed(&destination, version)
            .map_err(|error| invalid(format!("{error}; run `boon get {version} --force`")))?;
        if existing.artifacts != entry.artifacts {
            return Err(invalid("another download installed a different snapshot"));
        }
    } else if let Err(error) = fs::rename(&stage, &destination) {
        // Another process may have installed the same version after our check.
        if !installed(&destination, version).is_ok_and(|other| other.artifacts == entry.artifacts) {
            return Err(error.into());
        }
    }
    Ok(destination)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn upstream(version: &str) -> (Value, BTreeMap<String, Vec<u8>>) {
        let mut files: BTreeMap<String, Vec<u8>> = FILES
            .into_iter()
            .filter(|&name| name != "manifest.json")
            .map(|name| (name.into(), b"{}".to_vec()))
            .collect();
        let fingerprint = |bytes: &[u8]| json!({"bytes": bytes.len(), "sha256": format!("{:x}", Sha256::digest(bytes))});
        let artifacts: BTreeMap<_, _> = files
            .iter()
            .map(|(name, bytes)| (name, fingerprint(bytes)))
            .collect();
        let manifest = json!({"release_key": version, "client_version": version,
            "source": {"repository": SOURCE_REPO, "commit": "a".repeat(40)}, "artifacts": artifacts});
        files.insert(
            "manifest.json".into(),
            serde_json::to_vec(&manifest).unwrap(),
        );
        let artifacts: BTreeMap<_, _> = files
            .iter()
            .map(|(name, bytes)| {
                let mut artifact = fingerprint(bytes);
                artifact["url"] = json!(format!("{DOWNLOAD_URL}/{version}/{name}"));
                (name, artifact)
            })
            .collect();
        let entry = json!({"client_version": version, "snapshot": version,
            "released_at": "2026-09-25T00:00:00Z", "artifacts": artifacts});
        let snapshot = json!({"client_version": version,
            "released_at": "2026-09-25T00:00:00Z", "artifacts": artifacts});
        let index = json!({"source_repository": SOURCE_REPO, "latest": version,
            "versions": {version: entry}, "snapshots": {version: snapshot}});
        (
            index,
            files
                .into_iter()
                .map(|(name, bytes)| (format!("{DOWNLOAD_URL}/{version}/{name}"), bytes))
                .collect(),
        )
    }

    fn install(root: &Path, version: &str) -> PathBuf {
        let (index, files) = upstream(version);
        resolve(root, Some(version), &mut |url, _| {
            Ok(if url == INDEX_URL {
                serde_json::to_vec(&index).unwrap()
            } else {
                files[url].clone()
            })
        })
        .unwrap()
    }

    #[test]
    fn first_lookup_downloads_complete_release_then_reuses_offline() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join("cache");
        let (index, files) = upstream("42");
        let mut requests = Vec::new();
        let directory = resolve(&root, None, &mut |url, _| {
            requests.push(url.to_owned());
            Ok(if url == INDEX_URL {
                serde_json::to_vec(&index).unwrap()
            } else {
                files[url].clone()
            })
        })
        .unwrap();
        assert_eq!(requests.len(), 6);
        assert_eq!(directory, root.join("42"));
        assert_eq!(
            resolve(&root, None, &mut |_, _| panic!("offline")).unwrap(),
            directory
        );
        assert_eq!(
            resolve(&root, Some("42"), &mut |_, _| panic!("offline")).unwrap(),
            directory
        );
        assert_eq!(
            installed(&directory, "42").unwrap().metadata["released_at"],
            "2026-09-25T00:00:00Z"
        );
    }

    #[test]
    fn selects_newest_verified_version_and_honors_explicit_version() {
        let temp = tempfile::tempdir().unwrap();
        install(temp.path(), "9");
        let newest = install(temp.path(), "10");
        install(temp.path(), "11");
        fs::write(temp.path().join("11/heroes.json"), b"corrupt").unwrap();
        assert_eq!(
            resolve(temp.path(), None, &mut |_, _| panic!("offline")).unwrap(),
            newest
        );
        assert_eq!(
            resolve(temp.path(), Some("9"), &mut |_, _| panic!("offline")).unwrap(),
            temp.path().join("9")
        );
        assert!(
            resolve(temp.path(), Some("11"), &mut |_, _| panic!("offline"))
                .unwrap_err()
                .to_string()
                .contains("boon get 11 --force")
        );
        for version in ["", "../escape", "/tmp/escape", "latest"] {
            assert!(
                resolve(temp.path(), Some(version), &mut |_, _| panic!(
                    "invalid version"
                ))
                .is_err()
            );
        }
    }

    #[test]
    fn failed_download_never_installs_partial_data() {
        let temp = tempfile::tempdir().unwrap();
        let (index, mut files) = upstream("42");
        files.insert(
            format!("{DOWNLOAD_URL}/42/heroes.json"),
            b"corrupt".to_vec(),
        );
        assert!(
            resolve(temp.path(), None, &mut |url, _| Ok(if url == INDEX_URL {
                serde_json::to_vec(&index).unwrap()
            } else {
                files[url].clone()
            }))
            .is_err()
        );
        assert_eq!(fs::read_dir(temp.path()).unwrap().count(), 0);
    }

    #[test]
    fn backfill_only_index_uses_highest_client_version() {
        let temp = tempfile::tempdir().unwrap();
        let (mut index, files) = upstream("42");
        let (older, _) = upstream("9");
        index["versions"]["9"] = older["versions"]["9"].clone();
        index["latest"] = Value::Null;
        let directory = resolve(temp.path(), None, &mut |url, _| {
            Ok(if url == INDEX_URL {
                serde_json::to_vec(&index).unwrap()
            } else {
                files[url].clone()
            })
        })
        .unwrap();
        assert_eq!(directory, temp.path().join("42"));
    }

    #[test]
    fn unknown_version_does_not_fall_back_to_latest() {
        let temp = tempfile::tempdir().unwrap();
        let (index, _) = upstream("42");
        assert!(
            resolve(temp.path(), Some("41"), &mut |url, _| {
                assert_eq!(url, INDEX_URL);
                Ok(serde_json::to_vec(&index).unwrap())
            })
            .unwrap_err()
            .to_string()
            .contains("41 is not available")
        );
    }

    #[test]
    fn shared_snapshot_keeps_requested_version_and_python_receipt() {
        let temp = tempfile::tempdir().unwrap();
        let (mut index, files) = upstream("42");
        let mut entry = index["versions"]["42"].clone();
        entry["client_version"] = json!("43");
        index["versions"]["43"] = entry;
        index["latest"] = json!("43");
        let directory = resolve(temp.path(), None, &mut |url, _| {
            Ok(if url == INDEX_URL {
                serde_json::to_vec(&index).unwrap()
            } else {
                files[url].clone()
            })
        })
        .unwrap();
        assert_eq!(directory, temp.path().join("43"));
        assert_eq!(installed(&directory, "43").unwrap().snapshot, "42");
    }

    #[test]
    fn rejects_inconsistent_index_and_untrusted_download_urls() {
        for field in ["url", "sha256"] {
            let temp = tempfile::tempdir().unwrap();
            let (mut index, _) = upstream("42");
            index["versions"]["42"]["artifacts"]["heroes.json"][field] = json!("invalid");
            assert!(
                resolve(temp.path(), None, &mut |url, _| {
                    assert_eq!(url, INDEX_URL);
                    Ok(serde_json::to_vec(&index).unwrap())
                })
                .is_err()
            );
        }
    }
}

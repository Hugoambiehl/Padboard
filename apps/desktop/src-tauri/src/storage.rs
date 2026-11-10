//! Versioned JSON documents in the user config directory
//! (`~/.config/io.github.hugoambiehl.padboard` on Linux, `%APPDATA%\…` on Windows).
//!
//! Every file is `{ "version": N, ...fields }`. Loading upgrades older files
//! step by step through [`Document::migrate`]; writes are atomic (temporary
//! file, fsync, rename) so a crash never leaves a half-written file. Documents
//! holding secrets ([`Document::PRIVATE`]) are created with mode `0600` on
//! Unix (the directory itself is `0700`); on Windows the user profile ACLs
//! already restrict them to the user.
//!
//! An unreadable file (corrupt, or written by a newer Padboard) is moved aside
//! to `<file>.unreadable-<unix time>` and replaced by the default: the app
//! keeps working and nothing is silently overwritten.

use std::fs::{self, File, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::Serialize;
use serde::de::DeserializeOwned;
use serde_json::{Map, Value};

#[derive(Debug, thiserror::Error)]
pub enum StorageError {
    #[error("{path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
    #[error("{path}: invalid JSON: {source}")]
    Json {
        path: PathBuf,
        #[source]
        source: serde_json::Error,
    },
    #[error("{path}: missing or invalid \"version\"")]
    NoVersion { path: PathBuf },
    #[error("{path}: version {found} is newer than this Padboard (supports {supported})")]
    TooNew {
        path: PathBuf,
        found: u64,
        supported: u32,
    },
    #[error("{path}: cannot migrate from version {from}: {reason}")]
    Migration {
        path: PathBuf,
        from: u32,
        reason: String,
    },
}

/// A JSON document stored in the config directory.
pub trait Document: Serialize + DeserializeOwned {
    /// File name inside the config directory.
    const FILE: &'static str;
    /// Current version, written to the file.
    const VERSION: u32;
    /// `true` for files holding secrets (written `0600` on Unix).
    const PRIVATE: bool = false;

    /// Upgrades the fields of a version-`from` document to version `from + 1`.
    /// Only called for `from < VERSION`. Version 1 documents have nothing to
    /// migrate from, hence the default.
    ///
    /// # Errors
    /// A human-readable reason when the document cannot be upgraded.
    fn migrate(from: u32, fields: Map<String, Value>) -> Result<Map<String, Value>, String> {
        let _ = fields;
        Err(format!("no migration from version {from}"))
    }
}

/// The config directory. Cheap to clone.
#[derive(Debug, Clone)]
pub struct ConfigDir {
    root: PathBuf,
}

impl ConfigDir {
    /// Uses `root`, creating it (`0700` on Unix) if needed.
    ///
    /// # Errors
    /// If the directory cannot be created.
    pub fn open(root: impl Into<PathBuf>) -> Result<Self, StorageError> {
        let root = root.into();
        create_private_dir(&root).map_err(|source| StorageError::Io {
            path: root.clone(),
            source,
        })?;
        Ok(Self { root })
    }

    #[must_use]
    pub fn root(&self) -> &Path {
        &self.root
    }

    #[must_use]
    pub fn path_of<D: Document>(&self) -> PathBuf {
        self.root.join(D::FILE)
    }

    /// Loads a document; `Ok(None)` if the file does not exist.
    ///
    /// # Errors
    /// If the file exists but cannot be read, parsed or migrated.
    pub fn load<D: Document>(&self) -> Result<Option<D>, StorageError> {
        let path = self.path_of::<D>();
        let text = match fs::read_to_string(&path) {
            Ok(text) => text,
            Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(None),
            Err(source) => return Err(StorageError::Io { path, source }),
        };
        let json_error = |source| StorageError::Json {
            path: path.clone(),
            source,
        };
        let Value::Object(mut fields) = serde_json::from_str(&text).map_err(json_error)? else {
            return Err(StorageError::NoVersion { path });
        };
        let found = fields
            .remove("version")
            .and_then(|v| v.as_u64())
            .ok_or_else(|| StorageError::NoVersion { path: path.clone() })?;
        let Some(mut version) = u32::try_from(found).ok().filter(|v| *v <= D::VERSION) else {
            return Err(StorageError::TooNew {
                path,
                found,
                supported: D::VERSION,
            });
        };
        while version < D::VERSION {
            fields = D::migrate(version, fields).map_err(|reason| StorageError::Migration {
                path: path.clone(),
                from: version,
                reason,
            })?;
            version += 1;
        }
        let document = serde_json::from_value(Value::Object(fields)).map_err(json_error)?;
        if found != u64::from(D::VERSION) {
            tracing::info!(file = D::FILE, from = found, to = D::VERSION, "migrated");
            // Persist the upgrade so it happens only once; failing here is not fatal.
            if let Err(e) = self.save(&document) {
                tracing::warn!("cannot save the migrated {}: {e}", D::FILE);
            }
        }
        Ok(Some(document))
    }

    /// Loads a document, or creates it with `init` when it is missing or unreadable
    /// (an unreadable file is moved aside first, see the module docs).
    pub fn load_or_init<D: Document>(&self, init: impl FnOnce() -> D) -> D {
        match self.load::<D>() {
            Ok(Some(document)) => return document,
            Ok(None) => {}
            Err(e) => {
                tracing::error!("{e}");
                self.move_aside::<D>();
            }
        }
        let document = init();
        if let Err(e) = self.save(&document) {
            tracing::error!("cannot save {}: {e}", D::FILE);
        }
        document
    }

    /// Writes a document atomically.
    ///
    /// # Errors
    /// If the file cannot be written.
    pub fn save<D: Document>(&self, document: &D) -> Result<(), StorageError> {
        let path = self.path_of::<D>();
        let io_error = |source| StorageError::Io {
            path: path.clone(),
            source,
        };
        let mut fields = match serde_json::to_value(document) {
            Ok(Value::Object(fields)) => fields,
            Ok(_) => unreachable!("documents serialize to JSON objects"),
            Err(source) => {
                return Err(StorageError::Json {
                    path: path.clone(),
                    source,
                });
            }
        };
        fields.insert("version".into(), Value::from(D::VERSION));
        let mut text = serde_json::to_string_pretty(&Value::Object(fields)).map_err(|source| {
            StorageError::Json {
                path: path.clone(),
                source,
            }
        })?;
        text.push('\n');
        write_atomic(&path, text.as_bytes(), D::PRIVATE).map_err(io_error)
    }

    /// Reads a raw (non-JSON) file; `Ok(None)` if it does not exist.
    ///
    /// # Errors
    /// If the file exists but cannot be read.
    pub fn read_raw(&self, file: &str) -> Result<Option<Vec<u8>>, StorageError> {
        let path = self.root.join(file);
        match fs::read(&path) {
            Ok(bytes) => Ok(Some(bytes)),
            Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(None),
            Err(source) => Err(StorageError::Io { path, source }),
        }
    }

    /// Writes a raw secret file atomically, owner-only on Unix.
    ///
    /// # Errors
    /// If the file cannot be written.
    pub fn write_secret(&self, file: &str, bytes: &[u8]) -> Result<(), StorageError> {
        let path = self.root.join(file);
        write_atomic(&path, bytes, true).map_err(|source| StorageError::Io { path, source })
    }

    /// Moves an unusable file aside (see the module docs).
    pub fn move_raw_aside(&self, file: &str) {
        move_aside(&self.root, file);
    }

    fn move_aside<D: Document>(&self) {
        move_aside(&self.root, D::FILE);
    }
}

fn move_aside(root: &Path, file: &str) {
    let path = root.join(file);
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or_default();
    let aside = root.join(format!("{file}.unreadable-{stamp}"));
    match fs::rename(&path, &aside) {
        Ok(()) => tracing::warn!("moved the unreadable file to {}", aside.display()),
        Err(e) => tracing::error!("cannot move {} aside: {e}", path.display()),
    }
}

fn write_atomic(path: &Path, bytes: &[u8], private: bool) -> io::Result<()> {
    let tmp = path.with_extension("tmp");
    {
        let mut file = create_file(&tmp, private)?;
        file.write_all(bytes)?;
        file.sync_all()?;
    }
    fs::rename(&tmp, path)?;
    // Make the rename itself durable (best effort, Unix only).
    #[cfg(unix)]
    if let Some(dir) = path.parent() {
        let _ = File::open(dir).and_then(|d| d.sync_all());
    }
    Ok(())
}

fn create_file(path: &Path, private: bool) -> io::Result<File> {
    let mut options = OpenOptions::new();
    options.write(true).create(true).truncate(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(if private { 0o600 } else { 0o644 });
    }
    #[cfg(not(unix))]
    let _ = private;
    let file = options.open(path)?;
    // `mode` only applies on creation: fix a leftover temporary file too.
    #[cfg(unix)]
    if private {
        use std::os::unix::fs::PermissionsExt;
        file.set_permissions(fs::Permissions::from_mode(0o600))?;
    }
    Ok(file)
}

fn create_private_dir(path: &Path) -> io::Result<()> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::DirBuilderExt;
        match fs::DirBuilder::new()
            .recursive(true)
            .mode(0o700)
            .create(path)
        {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == io::ErrorKind::AlreadyExists && path.is_dir() => Ok(()),
            Err(e) => Err(e),
        }
    }
    #[cfg(not(unix))]
    fs::create_dir_all(path)
}

#[cfg(test)]
mod tests {
    use serde::Deserialize;

    use super::*;

    #[derive(Debug, PartialEq, Serialize, Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct Doc {
        display_name: String,
        count: u32,
    }

    impl Document for Doc {
        const FILE: &'static str = "doc.json";
        const VERSION: u32 = 3;
        const PRIVATE: bool = true;

        fn migrate(from: u32, mut f: Map<String, Value>) -> Result<Map<String, Value>, String> {
            match from {
                // v1 → v2: "name" renamed to "displayName".
                1 => {
                    let name = f.remove("name").ok_or("no name")?;
                    f.insert("displayName".into(), name);
                    Ok(f)
                }
                // v2 → v3: "count" added.
                2 => {
                    f.insert("count".into(), 0.into());
                    Ok(f)
                }
                _ => Err("unknown".into()),
            }
        }
    }

    fn dir() -> (tempfile::TempDir, ConfigDir) {
        let tmp = tempfile::tempdir().unwrap();
        let config = ConfigDir::open(tmp.path().join("padboard")).unwrap();
        (tmp, config)
    }

    fn doc() -> Doc {
        Doc {
            display_name: "PC".into(),
            count: 2,
        }
    }

    #[test]
    fn missing_file_loads_as_none() {
        let (_tmp, config) = dir();
        assert!(config.load::<Doc>().unwrap().is_none());
    }

    #[test]
    fn save_then_load_round_trips_with_version() {
        let (_tmp, config) = dir();
        config.save(&doc()).unwrap();
        assert_eq!(config.load::<Doc>().unwrap(), Some(doc()));
        let text = fs::read_to_string(config.path_of::<Doc>()).unwrap();
        assert!(text.contains("\"version\": 3"), "{text}");
        assert!(!config.root().join("doc.tmp").exists());
    }

    #[test]
    fn old_versions_are_migrated_step_by_step_and_rewritten() {
        let (_tmp, config) = dir();
        fs::write(
            config.path_of::<Doc>(),
            r#"{"version": 1, "name": "Old PC"}"#,
        )
        .unwrap();
        let loaded = config.load::<Doc>().unwrap().unwrap();
        assert_eq!(
            loaded,
            Doc {
                display_name: "Old PC".into(),
                count: 0
            }
        );
        let text = fs::read_to_string(config.path_of::<Doc>()).unwrap();
        assert!(text.contains("\"version\": 3"), "{text}");
    }

    #[test]
    fn newer_or_unversioned_files_are_errors() {
        let (_tmp, config) = dir();
        fs::write(config.path_of::<Doc>(), r#"{"version": 4}"#).unwrap();
        assert!(matches!(
            config.load::<Doc>(),
            Err(StorageError::TooNew { found: 4, .. })
        ));
        fs::write(config.path_of::<Doc>(), r#"{"displayName": "x"}"#).unwrap();
        assert!(matches!(
            config.load::<Doc>(),
            Err(StorageError::NoVersion { .. })
        ));
        fs::write(config.path_of::<Doc>(), "{oops").unwrap();
        assert!(matches!(
            config.load::<Doc>(),
            Err(StorageError::Json { .. })
        ));
    }

    #[test]
    fn unreadable_file_is_moved_aside_and_replaced() {
        let (_tmp, config) = dir();
        fs::write(config.path_of::<Doc>(), "{oops").unwrap();
        let loaded = config.load_or_init(doc);
        assert_eq!(loaded, doc());
        let aside: Vec<_> = fs::read_dir(config.root())
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
            .filter(|n| n.starts_with("doc.json.unreadable-"))
            .collect();
        assert_eq!(aside.len(), 1);
        let kept = fs::read_to_string(config.root().join(&aside[0])).unwrap();
        assert_eq!(kept, "{oops");
        assert_eq!(config.load::<Doc>().unwrap(), Some(doc()));
    }

    #[test]
    fn existing_document_is_not_reinitialized() {
        let (_tmp, config) = dir();
        config.save(&doc()).unwrap();
        let loaded = config.load_or_init(|| -> Doc { panic!("must not init") });
        assert_eq!(loaded, doc());
    }

    #[cfg(unix)]
    #[test]
    fn private_documents_and_directory_are_owner_only() {
        use std::os::unix::fs::PermissionsExt;
        let (_tmp, config) = dir();
        config.save(&doc()).unwrap();
        let mode = |p: &Path| fs::metadata(p).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode(&config.path_of::<Doc>()), 0o600);
        assert_eq!(mode(config.root()), 0o700);
    }
}

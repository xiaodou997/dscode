use std::fmt;
use std::fs::{self, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use toml_edit::{DocumentMut, Item, Table, value};

use crate::{DEFAULT_API_KEY_ENV, DEFAULT_BASE_URL, PROVIDER_ID};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OfficialCodexConfigStatus {
    pub config_exists: bool,
    pub active_provider: Option<String>,
    pub doustack_configured: bool,
    pub latest_backup: Option<PathBuf>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OfficialCodexConfigChange {
    pub changed: bool,
    pub config_path: PathBuf,
    pub backup_path: Option<PathBuf>,
}

#[derive(Debug)]
pub enum OfficialCodexConfigError {
    Read { path: PathBuf, source: io::Error },
    Parse { path: PathBuf, message: String },
    UnsafeSymlink(PathBuf),
    Write { path: PathBuf, source: io::Error },
    NoBackup(PathBuf),
    InvalidBackup(PathBuf),
}

impl fmt::Display for OfficialCodexConfigError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Read { path, source } => write!(f, "cannot read {}: {source}", path.display()),
            Self::Parse { path, message } => {
                write!(f, "cannot parse {}: {message}", path.display())
            }
            Self::UnsafeSymlink(path) => {
                write!(f, "refusing to replace symlinked config {}", path.display())
            }
            Self::Write { path, source } => write!(f, "cannot write {}: {source}", path.display()),
            Self::NoBackup(path) => {
                write!(f, "no DS Code config backup found in {}", path.display())
            }
            Self::InvalidBackup(path) => write!(f, "invalid DS Code backup at {}", path.display()),
        }
    }
}

impl std::error::Error for OfficialCodexConfigError {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OfficialCodexConfigManager {
    codex_home: PathBuf,
    backup_root: PathBuf,
}

impl OfficialCodexConfigManager {
    pub fn new(codex_home: PathBuf, backup_root: PathBuf) -> Self {
        Self {
            codex_home,
            backup_root,
        }
    }

    pub fn config_path(&self) -> PathBuf {
        self.codex_home.join("config.toml")
    }

    pub fn status(&self) -> Result<OfficialCodexConfigStatus, OfficialCodexConfigError> {
        let path = self.config_path();
        let contents = read_optional_config(&path)?;
        let document = contents
            .as_deref()
            .map(|contents| parse_document(&path, contents))
            .transpose()?;
        let active_provider = document
            .as_ref()
            .and_then(|document| document.get("model_provider"))
            .and_then(Item::as_str)
            .map(str::to_string);
        let doustack_configured = document
            .as_ref()
            .and_then(|document| document.get("model_providers"))
            .and_then(Item::as_table)
            .is_some_and(|providers| providers.contains_key(PROVIDER_ID));

        Ok(OfficialCodexConfigStatus {
            config_exists: contents.is_some(),
            active_provider,
            doustack_configured,
            latest_backup: self.latest_backup()?,
        })
    }

    pub fn preview_doustack_config(&self) -> Result<String, OfficialCodexConfigError> {
        let path = self.config_path();
        let contents = read_optional_config(&path)?;
        let mut document = contents
            .as_deref()
            .map(|contents| parse_document(&path, contents))
            .transpose()?
            .unwrap_or_default();
        merge_doustack_provider(&path, &mut document)?;
        Ok(render_document(document))
    }

    pub fn apply_doustack_config(
        &self,
    ) -> Result<OfficialCodexConfigChange, OfficialCodexConfigError> {
        let config_path = self.config_path();
        reject_symlink(&config_path)?;
        let previous = read_optional_config(&config_path)?;
        let mut document = previous
            .as_deref()
            .map(|contents| parse_document(&config_path, contents))
            .transpose()?
            .unwrap_or_default();
        merge_doustack_provider(&config_path, &mut document)?;
        let updated = render_document(document);

        if previous.as_deref() == Some(updated.as_str()) {
            return Ok(OfficialCodexConfigChange {
                changed: false,
                config_path,
                backup_path: None,
            });
        }

        let backup_path = self.backup_current(previous.as_deref())?;
        atomic_write(&config_path, updated.as_bytes())?;
        Ok(OfficialCodexConfigChange {
            changed: true,
            config_path,
            backup_path: Some(backup_path),
        })
    }

    pub fn restore_latest_backup(
        &self,
    ) -> Result<OfficialCodexConfigChange, OfficialCodexConfigError> {
        let config_path = self.config_path();
        reject_symlink(&config_path)?;
        let backup_path = self
            .latest_backup()?
            .ok_or_else(|| OfficialCodexConfigError::NoBackup(self.backup_root.clone()))?;
        let backup_config = backup_path.join("config.toml");
        let absent_marker = backup_path.join("config.absent");

        if backup_config.is_file() {
            let contents =
                fs::read(&backup_config).map_err(|source| OfficialCodexConfigError::Read {
                    path: backup_config,
                    source,
                })?;
            atomic_write(&config_path, &contents)?;
        } else if absent_marker.is_file() {
            match fs::remove_file(&config_path) {
                Ok(()) => {}
                Err(source) if source.kind() == io::ErrorKind::NotFound => {}
                Err(source) => {
                    return Err(OfficialCodexConfigError::Write {
                        path: config_path.clone(),
                        source,
                    });
                }
            }
        } else {
            return Err(OfficialCodexConfigError::InvalidBackup(backup_path));
        }

        Ok(OfficialCodexConfigChange {
            changed: true,
            config_path,
            backup_path: Some(backup_path),
        })
    }

    fn backup_current(&self, contents: Option<&str>) -> Result<PathBuf, OfficialCodexConfigError> {
        let backup_path = self.backup_root.join(backup_id());
        fs::create_dir_all(&backup_path).map_err(|source| OfficialCodexConfigError::Write {
            path: backup_path.clone(),
            source,
        })?;
        let (path, bytes) = match contents {
            Some(contents) => (backup_path.join("config.toml"), contents.as_bytes()),
            None => (backup_path.join("config.absent"), b"".as_slice()),
        };
        write_new_file(&path, bytes)?;
        Ok(backup_path)
    }

    fn latest_backup(&self) -> Result<Option<PathBuf>, OfficialCodexConfigError> {
        let entries = match fs::read_dir(&self.backup_root) {
            Ok(entries) => entries,
            Err(source) if source.kind() == io::ErrorKind::NotFound => return Ok(None),
            Err(source) => {
                return Err(OfficialCodexConfigError::Read {
                    path: self.backup_root.clone(),
                    source,
                });
            }
        };
        let mut backups = entries
            .filter_map(Result::ok)
            .map(|entry| entry.path())
            .filter(|path| path.is_dir())
            .collect::<Vec<_>>();
        backups.sort();
        Ok(backups.pop())
    }
}

fn merge_doustack_provider(
    path: &Path,
    document: &mut DocumentMut,
) -> Result<(), OfficialCodexConfigError> {
    if document
        .get("model_providers")
        .is_some_and(|item| !item.is_table())
    {
        return Err(OfficialCodexConfigError::Parse {
            path: path.to_path_buf(),
            message: "model_providers must be a TOML table".to_string(),
        });
    }

    document["model_provider"] = value(PROVIDER_ID);
    let providers = table_mut(document, "model_providers");
    let mut provider = Table::new();
    provider["name"] = value("OpenAI");
    provider["base_url"] = value(DEFAULT_BASE_URL);
    provider["env_key"] = value(DEFAULT_API_KEY_ENV);
    provider["wire_api"] = value("responses");
    provider["requires_openai_auth"] = value(false);
    provider["supports_websockets"] = value(false);
    providers[PROVIDER_ID] = Item::Table(provider);
    Ok(())
}

fn table_mut<'a>(document: &'a mut DocumentMut, key: &str) -> &'a mut Table {
    if !document.get(key).is_some_and(Item::is_table) {
        document[key] = Item::Table(Table::new());
    }
    document[key]
        .as_table_mut()
        .expect("document entry was initialized as a table")
}

fn read_optional_config(path: &Path) -> Result<Option<String>, OfficialCodexConfigError> {
    match fs::read_to_string(path) {
        Ok(contents) => Ok(Some(contents)),
        Err(source) if source.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(source) => Err(OfficialCodexConfigError::Read {
            path: path.to_path_buf(),
            source,
        }),
    }
}

fn parse_document(path: &Path, contents: &str) -> Result<DocumentMut, OfficialCodexConfigError> {
    contents
        .parse::<DocumentMut>()
        .map_err(|error| OfficialCodexConfigError::Parse {
            path: path.to_path_buf(),
            message: error.to_string(),
        })
}

fn render_document(document: DocumentMut) -> String {
    let mut output = document.to_string();
    if !output.ends_with('\n') {
        output.push('\n');
    }
    output
}

fn reject_symlink(path: &Path) -> Result<(), OfficialCodexConfigError> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_symlink() => {
            Err(OfficialCodexConfigError::UnsafeSymlink(path.to_path_buf()))
        }
        Ok(_) => Ok(()),
        Err(source) if source.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(source) => Err(OfficialCodexConfigError::Read {
            path: path.to_path_buf(),
            source,
        }),
    }
}

fn atomic_write(path: &Path, bytes: &[u8]) -> Result<(), OfficialCodexConfigError> {
    let parent = path
        .parent()
        .ok_or_else(|| OfficialCodexConfigError::Write {
            path: path.to_path_buf(),
            source: io::Error::new(
                io::ErrorKind::InvalidInput,
                "config has no parent directory",
            ),
        })?;
    fs::create_dir_all(parent).map_err(|source| OfficialCodexConfigError::Write {
        path: parent.to_path_buf(),
        source,
    })?;
    let temporary = parent.join(format!(
        ".config.toml.dscode-{}-{}.tmp",
        std::process::id(),
        timestamp_nanos()
    ));
    write_new_file(&temporary, bytes)?;
    fs::rename(&temporary, path).map_err(|source| {
        let _ = fs::remove_file(&temporary);
        OfficialCodexConfigError::Write {
            path: path.to_path_buf(),
            source,
        }
    })
}

fn write_new_file(path: &Path, bytes: &[u8]) -> Result<(), OfficialCodexConfigError> {
    let mut options = OpenOptions::new();
    options.create_new(true).write(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options
        .open(path)
        .map_err(|source| OfficialCodexConfigError::Write {
            path: path.to_path_buf(),
            source,
        })?;
    file.write_all(bytes)
        .and_then(|()| file.sync_all())
        .map_err(|source| OfficialCodexConfigError::Write {
            path: path.to_path_buf(),
            source,
        })
}

fn backup_id() -> String {
    format!(
        "{:030}-{}",
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos(),
        std::process::id()
    )
}

fn timestamp_nanos() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_manager(name: &str) -> (PathBuf, OfficialCodexConfigManager) {
        let root = std::env::temp_dir().join(format!(
            "dscode-official-{name}-{}-{}",
            std::process::id(),
            timestamp_nanos()
        ));
        let manager = OfficialCodexConfigManager::new(
            root.join(".codex"),
            root.join(".dscode/backups/official-codex"),
        );
        (root, manager)
    }

    #[test]
    fn merges_provider_without_discarding_user_config() {
        let (root, manager) = test_manager("merge");
        let config_path = manager.config_path();
        fs::create_dir_all(config_path.parent().expect("config parent")).expect("create home");
        fs::write(
            &config_path,
            "model = \"existing-model\"\n[sandbox_workspace_write]\nnetwork_access = true\n",
        )
        .expect("write config");

        let result = manager.apply_doustack_config().expect("apply config");
        let updated = fs::read_to_string(&config_path).expect("read config");

        assert!(result.changed);
        assert!(result.backup_path.is_some());
        assert!(updated.contains("model = \"existing-model\""));
        assert!(updated.contains("network_access = true"));
        assert!(updated.contains("model_provider = \"doustack\""));
        assert!(updated.contains("base_url = \"https://miao.313619.xyz\""));
        assert!(updated.contains("env_key = \"DOUSTACK_API_KEY\""));
        assert!(!updated.contains("sk-"));

        fs::remove_dir_all(root).expect("remove test directory");
    }

    #[test]
    fn restores_the_config_snapshot() {
        let (root, manager) = test_manager("restore");
        let config_path = manager.config_path();
        fs::create_dir_all(config_path.parent().expect("config parent")).expect("create home");
        let original = "model = \"official-model\"\n";
        fs::write(&config_path, original).expect("write original");

        manager.apply_doustack_config().expect("apply config");
        manager.restore_latest_backup().expect("restore backup");

        assert_eq!(
            fs::read_to_string(config_path).expect("read restored"),
            original
        );
        fs::remove_dir_all(root).expect("remove test directory");
    }

    #[test]
    fn restores_an_absent_config() {
        let (root, manager) = test_manager("absent");

        manager.apply_doustack_config().expect("apply config");
        assert!(manager.config_path().is_file());
        manager.restore_latest_backup().expect("restore absence");

        assert!(!manager.config_path().exists());
        fs::remove_dir_all(root).expect("remove test directory");
    }

    #[test]
    fn rejects_invalid_provider_structure_without_modifying_config() {
        let (root, manager) = test_manager("invalid-structure");
        let config_path = manager.config_path();
        fs::create_dir_all(config_path.parent().expect("config parent")).expect("create home");
        let original = "model_providers = \"custom-value\"\nmodel = \"existing-model\"\n";
        fs::write(&config_path, original).expect("write original");

        let error = manager
            .apply_doustack_config()
            .expect_err("invalid structure must be rejected");

        assert!(
            error
                .to_string()
                .contains("model_providers must be a TOML table")
        );
        assert_eq!(
            fs::read_to_string(&config_path).expect("read unchanged config"),
            original
        );
        assert!(manager.latest_backup().expect("read backups").is_none());
        fs::remove_dir_all(root).expect("remove test directory");
    }
}

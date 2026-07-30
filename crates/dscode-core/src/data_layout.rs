use std::fmt;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

pub const CODEX_HOME_ENV: &str = "CODEX_HOME";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DataLayout {
    root: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DataLayoutError {
    EmptyRoot,
    FilesystemRoot,
}

impl fmt::Display for DataLayoutError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyRoot => f.write_str("DS Code home cannot be empty"),
            Self::FilesystemRoot => f.write_str("DS Code home cannot be a filesystem root"),
        }
    }
}

impl std::error::Error for DataLayoutError {}

impl DataLayout {
    pub fn new(root: PathBuf) -> Result<Self, DataLayoutError> {
        if root.as_os_str().is_empty() {
            return Err(DataLayoutError::EmptyRoot);
        }
        if root.parent().is_none() {
            return Err(DataLayoutError::FilesystemRoot);
        }
        Ok(Self { root })
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn codex_home(&self) -> PathBuf {
        self.root.join("codex")
    }

    pub fn imports_dir(&self) -> PathBuf {
        self.root.join("imports")
    }

    pub fn backups_dir(&self) -> PathBuf {
        self.root.join("backups")
    }

    pub fn logs_dir(&self) -> PathBuf {
        self.root.join("logs")
    }

    pub fn cache_dir(&self) -> PathBuf {
        self.root.join("cache")
    }

    pub fn ensure(&self) -> io::Result<()> {
        for directory in self.required_directories() {
            fs::create_dir_all(directory)?;
        }
        Ok(())
    }

    fn required_directories(&self) -> [PathBuf; 6] {
        [
            self.root.clone(),
            self.codex_home(),
            self.imports_dir(),
            self.backups_dir(),
            self.logs_dir(),
            self.cache_dir(),
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn ensure_creates_the_isolated_directory_tree() {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock")
            .as_nanos();
        let root =
            std::env::temp_dir().join(format!("dscode-layout-{}-{unique}", std::process::id()));
        let layout = DataLayout::new(root.clone()).expect("valid test root");

        layout.ensure().expect("create data layout");

        assert!(layout.codex_home().is_dir());
        assert!(layout.imports_dir().is_dir());
        assert!(layout.backups_dir().is_dir());
        assert!(layout.logs_dir().is_dir());
        assert!(layout.cache_dir().is_dir());

        fs::remove_dir_all(root).expect("remove isolated test directory");
    }
}

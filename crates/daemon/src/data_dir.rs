use std::fs::{File, OpenOptions};
use std::path::{Path, PathBuf};

use fs4::fs_std::FileExt;

/// Every way acquiring or holding a data directory can fail, named
/// specifically rather than collapsed into one generic "IO error" —
/// the caller (and the person reading a failed test) can tell "someone
/// else already has this locked" apart from "the disk is unwritable"
/// without parsing a message string.
#[derive(Debug, thiserror::Error)]
pub enum DataDirError {
    #[error("could not create data directory at {path}: {source}")]
    Create {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("could not open lock file at {path}: {source}")]
    OpenLockFile {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error(
        "another instance already holds the lock on {path} — \
         only one daemon may run against a given data directory at a time"
    )]
    AlreadyLocked { path: PathBuf },
}

/// A data directory with an exclusive lock held for as long as this
/// value exists.
///
/// There is no public constructor other than [`DataDir::acquire`], and
/// it is the only way to obtain one — a `DataDir` value is proof that
/// the lock was actually acquired, not a path that merely *should*
/// have one. Any code holding a `DataDir` can rely on exclusive
/// ownership of that directory without re-checking, because it is
/// impossible to hold one without that being true. The lock is
/// released automatically when the value is dropped, including on a
/// panic — nothing to remember to clean up.
///
/// This is what makes "two instances started against the same
/// directory" fail cleanly (the second `acquire()` call returns
/// `Err(AlreadyLocked)`) rather than both processes racing to write
/// the same files.
#[derive(Debug)]
pub struct DataDir {
    path: PathBuf,
    // Held only for its Drop side effect — dropping the File releases
    // the OS-level advisory lock automatically. Never read directly.
    _lock_file: File,
}

impl DataDir {
    pub fn acquire(path: impl AsRef<Path>) -> Result<Self, DataDirError> {
        let path = path.as_ref().to_path_buf();

        std::fs::create_dir_all(&path)
            .map_err(|source| DataDirError::Create { path: path.clone(), source })?;

        let lock_path = path.join(".lock");
        let lock_file = OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(false) // content is irrelevant — only the OS lock matters
            .open(&lock_path)
            .map_err(|source| DataDirError::OpenLockFile { path: path.clone(), source })?;

        // Non-blocking: fails immediately with an error rather than
        // waiting, if another process already holds it — exactly the
        // "fail cleanly" behaviour the negative lifecycle test checks.
        lock_file
            .try_lock_exclusive()
            .map_err(|_| DataDirError::AlreadyLocked { path: path.clone() })?;

        Ok(Self { path, _lock_file: lock_file })
    }

    pub fn path(&self) -> &Path {
        &self.path
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn second_acquire_on_same_path_fails_cleanly() {
        let dir = tempfile::tempdir().expect("create temp dir");

        let first = DataDir::acquire(dir.path()).expect("first acquire must succeed");

        let second = DataDir::acquire(dir.path());
        assert!(
            matches!(second, Err(DataDirError::AlreadyLocked { .. })),
            "expected AlreadyLocked, got {second:?}"
        );

        drop(first);

        // Once released, acquiring again must succeed — the lock does
        // not leak past the DataDir's lifetime.
        let third = DataDir::acquire(dir.path());
        assert!(third.is_ok(), "expected re-acquire after release to succeed");
    }
}

use std::collections::HashSet;
use std::path::{Component, Path, PathBuf};
use std::sync::{Arc, Mutex};

use uuid::Uuid;

pub(crate) const CLEANUP_GRACE_SECONDS: i64 = 3600;

#[derive(Default)]
struct LeaseState {
    active: HashSet<PathBuf>,
    cleanup: HashSet<PathBuf>,
}

#[derive(Clone, Default)]
pub(crate) struct UploadLeaseRegistry {
    state: Arc<Mutex<LeaseState>>,
}

impl UploadLeaseRegistry {
    fn lock(&self) -> std::sync::MutexGuard<'_, LeaseState> {
        self.state.lock().unwrap_or_else(|error| error.into_inner())
    }

    fn register(&self, path: PathBuf) -> Result<UploadLease, std::io::Error> {
        let mut state = self.lock();
        if state.active.contains(&path)
            || state
                .cleanup
                .iter()
                .any(|claimed| path == *claimed || path.starts_with(claimed))
        {
            return Err(std::io::Error::new(
                std::io::ErrorKind::AlreadyExists,
                "upload path is already owned",
            ));
        }
        state.active.insert(path.clone());
        drop(state);
        Ok(UploadLease {
            registry: self.clone(),
            location: LeaseLocation::Owned(path),
        })
    }

    pub(crate) fn claim_cleanup(&self, path: &Path) -> Option<CleanupLease> {
        let mut state = self.lock();
        if state
            .active
            .iter()
            .any(|active| active == path || active.starts_with(path))
            || state.cleanup.iter().any(|claimed| {
                claimed == path || claimed.starts_with(path) || path.starts_with(claimed)
            })
        {
            return None;
        }
        state.cleanup.insert(path.to_path_buf());
        Some(CleanupLease {
            registry: self.clone(),
            path: path.to_path_buf(),
        })
    }
}

struct UploadLease {
    registry: UploadLeaseRegistry,
    location: LeaseLocation,
}

enum LeaseLocation {
    Owned(PathBuf),
    Transferring {
        source: PathBuf,
        destination: PathBuf,
    },
}

impl UploadLease {
    fn reserve(&mut self, destination: &Path) -> Result<(), String> {
        let LeaseLocation::Owned(source) = &self.location else {
            return Err("Upload already has a reserved destination".to_string());
        };
        let mut state = self.registry.lock();
        if state.active.contains(destination)
            || state
                .cleanup
                .iter()
                .any(|claimed| destination == claimed || destination.starts_with(claimed))
        {
            return Err("Attachment destination is already owned".to_string());
        }
        state.active.insert(destination.to_path_buf());
        self.location = LeaseLocation::Transferring {
            source: source.clone(),
            destination: destination.to_path_buf(),
        };
        Ok(())
    }

    fn finish_transfer(&mut self) {
        let LeaseLocation::Transferring {
            source,
            destination,
        } = &self.location
        else {
            return;
        };
        let mut state = self.registry.lock();
        state.active.remove(source);
        self.location = LeaseLocation::Owned(destination.clone());
    }

    fn cancel_transfer(&mut self) {
        let LeaseLocation::Transferring {
            source,
            destination,
        } = &self.location
        else {
            return;
        };
        self.registry.lock().active.remove(destination);
        self.location = LeaseLocation::Owned(source.clone());
    }
}

impl Drop for UploadLease {
    fn drop(&mut self) {
        let mut state = self.registry.lock();
        match &self.location {
            LeaseLocation::Owned(path) => {
                state.active.remove(path);
            }
            LeaseLocation::Transferring {
                source,
                destination,
            } => {
                state.active.remove(source);
                state.active.remove(destination);
            }
        }
    }
}

pub(crate) struct CleanupLease {
    registry: UploadLeaseRegistry,
    path: PathBuf,
}

impl Drop for CleanupLease {
    fn drop(&mut self) {
        self.registry.lock().cleanup.remove(&self.path);
    }
}

pub(crate) async fn old_enough_for_cleanup(path: &Path, now: i64) -> bool {
    let Ok(modified) = tokio::fs::metadata(path)
        .await
        .and_then(|value| value.modified())
    else {
        return false;
    };
    let Ok(modified) = modified.duration_since(std::time::UNIX_EPOCH) else {
        return false;
    };
    modified.as_secs() as i64 <= now.saturating_sub(CLEANUP_GRACE_SECONDS)
}

pub(crate) struct StagedUpload {
    path: PathBuf,
    pub(crate) filename: String,
    pub(crate) storage_key: String,
    pub(crate) size_bytes: i64,
    pub(crate) digest: String,
    state: UploadState,
}

enum UploadState {
    Staged(UploadLease),
    Promoted(UploadLease),
    Committed,
}

impl StagedUpload {
    pub(crate) async fn create(
        data_dir: &Path,
        leases: UploadLeaseRegistry,
        filename: String,
    ) -> Result<Self, std::io::Error> {
        let staging = data_dir.join("attachments").join(".staging");
        tokio::fs::create_dir_all(&staging).await?;
        let path = staging.join(format!("upload-{}", Uuid::new_v4().simple()));
        let lease = leases.register(path.clone())?;
        Ok(Self {
            path,
            filename,
            storage_key: Uuid::new_v4().simple().to_string(),
            size_bytes: 0,
            digest: String::new(),
            state: UploadState::Staged(lease),
        })
    }

    pub(crate) fn path(&self) -> &Path {
        &self.path
    }

    pub(crate) async fn promote(&mut self, data_dir: &Path, paste_id: &str) -> Result<(), String> {
        let destination = attachment_path(data_dir, paste_id, &self.storage_key)?;
        let UploadState::Staged(lease) = &mut self.state else {
            return Err("Only a staged upload can be promoted".to_string());
        };
        lease.reserve(&destination)?;
        let parent = destination
            .parent()
            .ok_or_else(|| "Attachment destination has no parent".to_string())?;
        if let Err(error) = tokio::fs::create_dir_all(parent).await {
            lease.cancel_transfer();
            return Err(error.to_string());
        }
        if let Err(error) = tokio::fs::rename(&self.path, &destination).await {
            lease.cancel_transfer();
            return Err(error.to_string());
        }
        lease.finish_transfer();
        self.path = destination;
        // Rename transferred ownership even if the subsequent timestamp refresh fails.
        if let UploadState::Staged(lease) =
            std::mem::replace(&mut self.state, UploadState::Committed)
        {
            self.state = UploadState::Promoted(lease);
        }
        std::fs::OpenOptions::new()
            .write(true)
            .open(&self.path)
            .and_then(|file| file.set_modified(std::time::SystemTime::now()))
            .map_err(|error| error.to_string())?;
        Ok(())
    }

    pub(crate) fn commit(&mut self) -> Result<(), String> {
        match &self.state {
            UploadState::Promoted(_) => {
                // Releasing this lease follows, never precedes, metadata commit.
                if let UploadState::Promoted(lease) =
                    std::mem::replace(&mut self.state, UploadState::Committed)
                {
                    drop(lease);
                }
                Ok(())
            }
            UploadState::Committed => Ok(()),
            UploadState::Staged(_) => Err("An upload cannot commit before promotion".to_string()),
        }
    }
}

impl Drop for StagedUpload {
    fn drop(&mut self) {
        if !matches!(self.state, UploadState::Committed) {
            let _ = std::fs::remove_file(&self.path);
        }
    }
}

pub(crate) fn attachment_path(
    data_dir: &Path,
    paste_id: &str,
    name: &str,
) -> Result<PathBuf, String> {
    let safe_component = |value: &str| {
        let mut components = Path::new(value).components();
        matches!(components.next(), Some(Component::Normal(_))) && components.next().is_none()
    };
    if !safe_component(paste_id) || !safe_component(name) || name.starts_with('.') {
        return Err("Unsafe attachment metadata".to_string());
    }
    Ok(data_dir.join("attachments").join(paste_id).join(name))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn active_and_reserved_paths_cannot_be_claimed_for_cleanup() {
        let registry = UploadLeaseRegistry::default();
        let staging = PathBuf::from("attachments/.staging/upload-a");
        let destination = PathBuf::from("attachments/paste/storage-key");
        let mut upload = registry.register(staging.clone()).unwrap();

        assert!(registry.claim_cleanup(&staging).is_none());
        assert!(registry
            .claim_cleanup(Path::new("attachments/.staging"))
            .is_none());
        upload.reserve(&destination).unwrap();
        assert!(registry
            .claim_cleanup(Path::new("attachments/paste"))
            .is_none());
        upload.finish_transfer();
        drop(upload);

        assert!(registry.claim_cleanup(&destination).is_some());
    }

    #[test]
    fn cancelled_transfer_releases_only_its_reservation() {
        let registry = UploadLeaseRegistry::default();
        let staging = PathBuf::from("attachments/.staging/example");
        let destination = PathBuf::from("attachments/example/file");
        let mut lease = registry.register(staging.clone()).unwrap();
        lease.reserve(&destination).unwrap();
        assert!(lease.reserve(Path::new("attachments/other/file")).is_err());
        lease.cancel_transfer();
        assert!(registry.claim_cleanup(&destination).is_some());
        assert!(registry.claim_cleanup(&staging).is_none());
        lease.reserve(&destination).unwrap();
        drop(lease);
        assert!(registry.claim_cleanup(&staging).is_some());
        assert!(registry.claim_cleanup(&destination).is_some());
    }

    #[actix_web::test]
    async fn uploads_require_promotion_before_commit_and_cannot_promote_twice() {
        let directory = tempfile::tempdir().unwrap();
        let registry = UploadLeaseRegistry::default();
        let mut upload =
            StagedUpload::create(directory.path(), registry.clone(), "example.txt".into())
                .await
                .unwrap();
        tokio::fs::write(upload.path(), b"example").await.unwrap();
        assert!(upload.commit().is_err());
        upload.promote(directory.path(), "example").await.unwrap();
        let path = upload.path().to_path_buf();
        assert!(registry.claim_cleanup(&path).is_none());
        assert!(upload.promote(directory.path(), "other").await.is_err());
        upload.commit().unwrap();
        upload.commit().unwrap();
        assert!(registry.claim_cleanup(&path).is_some());
        drop(upload);
        assert!(path.exists());
    }

    #[actix_web::test]
    async fn failed_or_abandoned_uploads_release_ownership_and_remove_bytes() {
        let directory = tempfile::tempdir().unwrap();
        let registry = UploadLeaseRegistry::default();
        let mut upload =
            StagedUpload::create(directory.path(), registry.clone(), "example.txt".into())
                .await
                .unwrap();
        let staging = upload.path().to_path_buf();
        tokio::fs::write(&staging, b"example").await.unwrap();
        upload.promote(directory.path(), "example").await.unwrap();
        let final_path = upload.path().to_path_buf();
        assert!(registry.claim_cleanup(&staging).is_some());
        drop(upload);
        assert!(!final_path.exists());
        assert!(registry.claim_cleanup(&final_path).is_some());
    }
}

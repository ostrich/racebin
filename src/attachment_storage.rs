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
            path,
            reserved: None,
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
    path: PathBuf,
    reserved: Option<PathBuf>,
}

impl UploadLease {
    fn reserve(&mut self, destination: &Path) -> Result<(), String> {
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
        self.reserved = Some(destination.to_path_buf());
        Ok(())
    }

    fn finish_transfer(&mut self) {
        let Some(destination) = self.reserved.take() else {
            return;
        };
        let mut state = self.registry.lock();
        state.active.remove(&self.path);
        self.path = destination;
    }

    fn cancel_transfer(&mut self) {
        let Some(destination) = self.reserved.take() else {
            return;
        };
        self.registry.lock().active.remove(&destination);
    }
}

impl Drop for UploadLease {
    fn drop(&mut self) {
        let mut state = self.registry.lock();
        state.active.remove(&self.path);
        if let Some(destination) = self.reserved.take() {
            state.active.remove(&destination);
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
    pub(crate) path: PathBuf,
    pub(crate) filename: String,
    pub(crate) storage_key: String,
    pub(crate) size_bytes: i64,
    pub(crate) digest: String,
    lease: Option<UploadLease>,
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
            lease: Some(lease),
        })
    }

    pub(crate) fn path(&self) -> &Path {
        &self.path
    }

    pub(crate) async fn promote(&mut self, data_dir: &Path, paste_id: &str) -> Result<(), String> {
        let destination = attachment_path(data_dir, paste_id, &self.storage_key)?;
        let lease = self
            .lease
            .as_mut()
            .ok_or_else(|| "Upload lease is no longer active".to_string())?;
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
        std::fs::OpenOptions::new()
            .write(true)
            .open(&self.path)
            .and_then(|file| file.set_modified(std::time::SystemTime::now()))
            .map_err(|error| error.to_string())?;
        Ok(())
    }

    pub(crate) fn commit(&mut self) {
        self.path = PathBuf::new();
        self.lease.take();
    }
}

impl Drop for StagedUpload {
    fn drop(&mut self) {
        if !self.path.as_os_str().is_empty() {
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
}

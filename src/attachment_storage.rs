use std::path::{Component, Path, PathBuf};

use uuid::Uuid;

pub(crate) struct StagedUpload {
    pub(crate) path: PathBuf,
    pub(crate) filename: String,
    pub(crate) storage_key: String,
    pub(crate) size_bytes: i64,
    pub(crate) digest: String,
}

impl StagedUpload {
    pub(crate) async fn create(data_dir: &Path, filename: String) -> Result<Self, std::io::Error> {
        let staging = data_dir.join("attachments").join(".staging");
        tokio::fs::create_dir_all(&staging).await?;
        Ok(Self {
            path: staging.join(format!("upload-{}", Uuid::new_v4().simple())),
            filename,
            storage_key: Uuid::new_v4().simple().to_string(),
            size_bytes: 0,
            digest: String::new(),
        })
    }

    pub(crate) fn path(&self) -> &Path {
        &self.path
    }

    pub(crate) async fn promote(&mut self, data_dir: &Path, paste_id: &str) -> Result<(), String> {
        let destination = attachment_path(data_dir, paste_id, &self.storage_key)?;
        let parent = destination
            .parent()
            .ok_or_else(|| "Attachment destination has no parent".to_string())?;
        tokio::fs::create_dir_all(parent)
            .await
            .map_err(|error| error.to_string())?;
        tokio::fs::rename(&self.path, &destination)
            .await
            .map_err(|error| error.to_string())?;
        self.path = destination;
        Ok(())
    }

    pub(crate) fn commit(&mut self) {
        self.path = PathBuf::new();
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

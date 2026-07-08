use crypto::{decrypt_blob, encrypt_blob};
use sha2::{Digest, Sha256};
use std::collections::HashSet;
use std::path::PathBuf;
use tokio::fs;
use tokio::io::AsyncWriteExt;

/// Content-addressed blob store for deduplicated, encrypted attachment storage.
/// Files are named by their SHA-256 hash, allowing automatic deduplication on write.
#[derive(Debug, Clone)]
pub struct BlobStore {
    base_dir: PathBuf,
    key: [u8; 32],
}

impl BlobStore {
    pub fn new(base_dir: PathBuf, key: [u8; 32]) -> Self {
        Self { base_dir, key }
    }

    pub fn base_dir(&self) -> &PathBuf {
        &self.base_dir
    }

    pub async fn init(&self) -> Result<(), std::io::Error> {
        fs::create_dir_all(&self.base_dir).await?;
        Ok(())
    }

    /// Saves data to disk using its SHA-256 hash as the filename.
    /// If the file already exists, it skips writing to provide automatic content-addressed deduplication.
    pub async fn save(&self, data: &[u8]) -> Result<String, std::io::Error> {
        let mut hasher = Sha256::new();
        hasher.update(data);
        let hash = format!("{:x}", hasher.finalize());
        let path = self.base_dir.join(&hash);
        if !path.exists() {
            let encrypted = encrypt_blob(&self.key, data)
                .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
            let mut file = fs::File::create(&path).await?;
            file.write_all(&encrypted).await?;
        }
        Ok(hash)
    }

    pub async fn load(&self, hash: &str) -> Result<Vec<u8>, std::io::Error> {
        let path = self.base_dir.join(hash);
        let encrypted = fs::read(path).await?;
        decrypt_blob(&self.key, &encrypted)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))
    }

    pub fn path_for(&self, hash: &str) -> PathBuf {
        self.base_dir.join(hash)
    }

    pub async fn delete(&self, hash: &str) -> Result<(), std::io::Error> {
        let path = self.base_dir.join(hash);
        if path.exists() {
            fs::remove_file(path).await?;
        }
        Ok(())
    }

    /// Deletes any blobs on disk that are not present in the provided `active_hashes` set.
    /// Returns the number of blobs successfully deleted.
    pub async fn garbage_collect(
        &self,
        active_hashes: &HashSet<String>,
    ) -> Result<usize, std::io::Error> {
        let mut deleted = 0;
        let mut entries = fs::read_dir(&self.base_dir).await?;
        while let Some(entry) = entries.next_entry().await? {
            let path = entry.path();
            if path.is_file() {
                if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                    if !active_hashes.contains(name) {
                        if fs::remove_file(&path).await.is_ok() {
                            deleted += 1;
                        }
                    }
                }
            }
        }
        Ok(deleted)
    }
}

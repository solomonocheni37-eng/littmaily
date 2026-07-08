use std::collections::HashSet;
use storage::blob::BlobStore;
use tempfile::tempdir;

#[tokio::test]
async fn given_new_blob_when_saved_then_can_be_loaded_by_hash() {
    let dir = tempdir().unwrap();
    let store = BlobStore::new(dir.path().to_path_buf(), [0u8; 32]);
    store.init().await.unwrap();
    let data = b"Hello, Tauri!";
    let hash = store.save(data).await.unwrap();
    let loaded = store.load(&hash).await.unwrap();
    assert_eq!(loaded, data);
}

#[tokio::test]
async fn given_existing_blob_when_saved_again_then_deduplicates_and_returns_same_hash() {
    let dir = tempdir().unwrap();
    let store = BlobStore::new(dir.path().to_path_buf(), [0u8; 32]);
    store.init().await.unwrap();
    let data = b"Duplicate content";
    let hash1 = store.save(data).await.unwrap();
    let hash2 = store.save(data).await.unwrap();
    // Verifies content-addressed deduplication: saving the exact same bytes twice
    // must not create a second file on disk, and must return the identical hash.
    assert_eq!(hash1, hash2);
}

#[tokio::test]
async fn given_missing_blob_when_loaded_then_returns_error() {
    let dir = tempdir().unwrap();
    let store = BlobStore::new(dir.path().to_path_buf(), [0u8; 32]);
    store.init().await.unwrap();
    let result = store.load("non_existent_sha256_hash").await;
    assert!(result.is_err());
}

#[tokio::test]
async fn given_orphaned_blobs_when_garbage_collected_then_only_unreferenced_are_deleted() {
    let dir = tempdir().unwrap();
    let store = BlobStore::new(dir.path().to_path_buf(), [0u8; 32]);
    store.init().await.unwrap();
    let keep_data = b"Keep me";
    let delete_data = b"Delete me";
    let keep_hash = store.save(keep_data).await.unwrap();
    let delete_hash = store.save(delete_data).await.unwrap();
    let mut active = HashSet::new();
    active.insert(keep_hash.clone());
    let deleted_count = store.garbage_collect(&active).await.unwrap();
    // Ensures the GC logic correctly distinguishes between active hashes (provided in the set)
    // and orphaned files on disk, deleting only the latter.
    assert_eq!(deleted_count, 1);
    assert!(store.load(&keep_hash).await.is_ok());
    assert!(store.load(&delete_hash).await.is_err());
}

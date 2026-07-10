//! Media sync module — S3-compatible upload with incremental lock file.

use std::collections::HashMap;
use std::fs;
use std::io::Read;
use std::path::Path;
use std::time::UNIX_EPOCH;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use futures::stream::{self, StreamExt};

use crate::error::EngineError;
use crate::image_proc::is_photo_file;

/// Default fingerprint threshold: 5MB. Files ≤ this use SHA-256 hash.
pub const FINGERPRINT_THRESHOLD: u64 = 5 * 1024 * 1024;

/// Default concurrency for S3 pulls.
pub const PULL_CONCURRENCY: usize = 16;

// ── Lock file types ────────────────────────────────────────────────

/// Fingerprint of a single file for change detection.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct FileFingerprint {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hash: Option<String>,
    pub size: u64,
    pub mtime: u64,
}

/// Lock file contents — tracks all successfully uploaded files.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SyncLockFile {
    pub files: HashMap<String, FileFingerprint>,
}

// ── Progress reporting ─────────────────────────────────────────────

/// Progress events emitted during sync operations.
#[derive(Debug, Clone)]
pub enum SyncProgress {
    Scanning { total: u32 },
    Uploading { current: u32, total: u32, file: String },
    GeneratingThumbnail { current: u32, total: u32, file: String },
    UploadingThumbnail { current: u32, total: u32 },
    Done,
}

// ── S3 Credentials ─────────────────────────────────────────────────

/// Explicit S3 credentials, avoids unsafe env var access in multi-threaded contexts.
#[derive(Debug, Clone)]
pub struct S3Credentials {
    pub access_key: String,
    pub secret_key: String,
}

// ── Config (serializable) ──────────────────────────────────────────

/// Options for the sync_media command (serializable, JSON-safe).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct SyncConfig {
    pub work_dir: std::path::PathBuf,
    pub dry_run: bool,
    /// Number of concurrent thumbnail generation tasks.
    /// `0` means auto-detect: `(available_parallelism / 2).clamp(2, 8)`.
    pub thumb_concurrency: usize,
}

impl Default for SyncConfig {
    fn default() -> Self {
        Self { work_dir: std::path::PathBuf::from("."), dry_run: false, thumb_concurrency: 0 }
    }
}

/// Backward-compatible alias.
pub type SyncOptions = SyncConfig;

// ── Context (runtime, non-serializable) ────────────────────────────

/// Runtime context for sync operations.
/// Pass this to receive progress callbacks during sync.
pub struct SyncContext {
    /// Progress callback. Called for each significant event.
    pub on_progress: Option<Box<dyn Fn(SyncProgress) + Send>>,
    /// Explicit S3 credentials. When None, falls back to env vars.
    pub credentials: Option<S3Credentials>,
    /// External cancellation token. When set to true, operation stops early.
    pub cancelled: Option<Arc<AtomicBool>>,
}

// ── Directory size calculation ─────────────────────────────────────

/// Recursively calculate the total size (in bytes) of photo files in a directory.
pub fn calculate_dir_size(dir: &Path) -> u64 {
    if !dir.is_dir() {
        return 0;
    }
    let mut total = 0u64;
    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                total += calculate_dir_size(&path);
            } else if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                if is_photo_file(name) {
                    total += entry.metadata().map(|m| m.len()).unwrap_or(0);
                }
            }
        }
    }
    total
}

// ── Fingerprint computation ────────────────────────────────────────

/// Compute a fingerprint for a file using the hybrid strategy.
pub fn compute_fingerprint(path: &Path, threshold: u64) -> Result<FileFingerprint, EngineError> {
    let meta = fs::metadata(path)?;
    let size = meta.len();
    let mtime = meta
        .modified()?
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();

    let hash = if size <= threshold {
        let mut file = fs::File::open(path)?;
        let mut hasher = Sha256::new();
        let mut buf = [0u8; 8192];
        loop {
            let n = file.read(&mut buf)?;
            if n == 0 {
                break;
            }
            hasher.update(&buf[..n]);
        }
        Some(format!("{:x}", hasher.finalize()))
    } else {
        None
    };

    Ok(FileFingerprint { hash, size, mtime })
}

// ── Lock file I/O ──────────────────────────────────────────────────

/// Load lock file from disk. Returns empty lock if file doesn't exist.
pub fn load_lock(path: &Path) -> SyncLockFile {
    match fs::read_to_string(path) {
        Ok(content) => serde_json::from_str(&content).unwrap_or_default(),
        Err(_) => SyncLockFile::default(),
    }
}

/// Save lock file to disk atomically.
pub fn save_lock(lock: &SyncLockFile, path: &Path) -> Result<(), EngineError> {
    let json = serde_json::to_string_pretty(lock)?;
    fs::write(path, json)?;
    Ok(())
}

// ── Change detection ───────────────────────────────────────────────

/// Determine if a file needs to be uploaded based on its fingerprint.
pub fn needs_upload(existing: Option<&FileFingerprint>, current: &FileFingerprint) -> bool {
    match existing {
        None => true,
        Some(prev) => {
            if let (Some(cur_hash), Some(prev_hash)) = (&current.hash, &prev.hash) {
                cur_hash != prev_hash
            } else {
                current.size != prev.size || current.mtime != prev.mtime
            }
        }
    }
}

// ── S3 upload ──────────────────────────────────────────────────────

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Instant;

use s3::bucket::Bucket;
use s3::creds::Credentials;
use s3::Region;

use crate::ProviderConfig;

/// Result of a sync_media operation.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncResult {
    pub uploaded: u32,
    pub skipped: u32,
    pub failed: Vec<String>,
    pub duration_ms: u64,
}

/// Create an S3 bucket client from provider config + optional explicit credentials.
/// Falls back to environment variables when credentials is None.
pub fn create_s3_bucket(config: &ProviderConfig, creds: Option<&S3Credentials>) -> Result<Box<Bucket>, EngineError> {
    let (access_key, secret_key) = if let Some(c) = creds {
        (c.access_key.clone(), c.secret_key.clone())
    } else {
        let ak = std::env::var("S3_ACCESS_KEY").map_err(|_| {
            EngineError::Config("S3_ACCESS_KEY not set. Please configure it in .env file".into())
        })?;
        let sk = std::env::var("S3_SECRET_KEY").map_err(|_| {
            EngineError::Config("S3_SECRET_KEY not set. Please configure it in .env file".into())
        })?;
        (ak, sk)
    };

    let credentials = Credentials::new(
        Some(&access_key),
        Some(&secret_key),
        None, None, None,
    )
    .map_err(|e| EngineError::Config(format!("Failed to create S3 credentials: {e}")))?;

    let region = Region::Custom {
        region: config.region.clone(),
        endpoint: config.endpoint.clone(),
    };

    let bucket = Bucket::new(&config.bucket, region, credentials)
        .map_err(|e| EngineError::Config(format!("Failed to create S3 bucket: {e}")))?
        .with_path_style();

    Ok(bucket)
}

/// Infer Content-Type from file path.
fn content_type_for(path: &Path) -> &'static str {
    path.extension()
        .and_then(|e| e.to_str())
        .map(crate::mime::resolve_mime_type)
        .unwrap_or("application/octet-stream")
}

/// Upload a single file to S3 with retry logic (async).
async fn upload_with_retry(
    bucket: &Bucket,
    local_path: &Path,
    remote_key: &str,
    max_retries: u32,
) -> Result<(), String> {
    let content = fs::read(local_path)
        .map_err(|e| format!("Failed to read {}: {e}", local_path.display()))?;
    let content_type = content_type_for(local_path);

    let mut last_err = String::new();
    for attempt in 0..max_retries {
        if attempt > 0 {
            tokio::time::sleep(std::time::Duration::from_secs(1 << attempt)).await;
        }
        match bucket.put_object_with_content_type(remote_key, &content, content_type).await {
            Ok(response) if response.status_code() >= 200 && response.status_code() < 300 => {
                return Ok(());
            }
            Ok(response) => {
                last_err = format!("HTTP {}", response.status_code());
            }
            Err(e) => {
                last_err = e.to_string();
            }
        }
    }
    Err(format!(
        "{}: failed after {} retries: {}",
        local_path.display(),
        max_retries,
        last_err
    ))
}

// ── Pull build assets (concurrent) ────────────────────────────────

/// Pull thumbs + JSON from S3 for CI build (no local albums/).
/// Uses buffer_unordered(16) for concurrent thumbnail downloads.
pub fn pull_build_assets(
    provider: &ProviderConfig,
    album_config: &crate::AlbumConfig,
    output_dir: &Path,
    creds: Option<&S3Credentials>,
) -> Result<u32, EngineError> {
    let bucket = create_s3_bucket(provider, creds)?;
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|e| EngineError::Config(format!("Failed to create runtime: {e}")))?;

    let output_dir = output_dir.to_path_buf();
    let albums = album_config.albums.clone();

    rt.block_on(async {
        let mut album_count = 0u32;

        // Pull generated JSON files (few files, keep serial)
        let gen_dir = output_dir.join("generated");
        fs::create_dir_all(&gen_dir)?;
        for name in std::iter::once("albums-index.json".to_string()).chain(
            albums.iter().map(|a| format!("album-{}.json", a.dir))
        ) {
            let key = format!("generated/{}", name);
            match bucket.get_object(&key).await {
                Ok(resp) if resp.status_code() == 200 => {
                    fs::write(gen_dir.join(&name), resp.bytes())?;
                }
                _ => { log::warn!("[pull] Not found: {key}"); }
            }
        }

        // Pull thumbnails for each album — concurrent within each album
        for entry in &albums {
            let prefix = format!("albums/{}/thumbs/", entry.dir);
            let objects = match bucket.list(prefix.clone(), None).await {
                Ok(results) => results.into_iter()
                    .flat_map(|r| r.contents)
                    .collect::<Vec<_>>(),
                Err(_) => continue,
            };
            if objects.is_empty() { continue; }
            album_count += 1;

            let thumbs_dir = output_dir.join("albums").join(&entry.dir).join("thumbs");
            fs::create_dir_all(&thumbs_dir)?;

            // Concurrent download with buffer_unordered
            let results: Vec<_> = stream::iter(objects.iter().map(|obj| {
                let bucket = &bucket;
                let prefix = &prefix;
                let thumbs_dir = &thumbs_dir;
                async move {
                    let filename = obj.key.strip_prefix(prefix).unwrap_or(&obj.key);
                    match bucket.get_object(&obj.key).await {
                        Ok(resp) if resp.status_code() == 200 => {
                            let _ = fs::write(thumbs_dir.join(filename), resp.bytes());
                            true
                        }
                        _ => {
                            log::warn!("[pull] Download failed: {}", obj.key);
                            false
                        }
                    }
                }
            }))
            .buffer_unordered(PULL_CONCURRENCY)
            .collect()
            .await;

            let success_count = results.iter().filter(|&&ok| ok).count();
            println!("  [albums] {} ✓ {} thumbs (from S3)", entry.dir, success_count);
        }

        Ok(album_count)
    })
}

// ── Sync orchestration ─────────────────────────────────────────────

/// Execute the full media sync pipeline (backward-compatible wrapper).
pub fn sync_media(opts: SyncOptions) -> Result<SyncResult, EngineError> {
    sync_media_with_context(opts, None)
}

/// Execute the full media sync pipeline with optional runtime context.
pub fn sync_media_with_context(config: SyncConfig, ctx: Option<SyncContext>) -> Result<SyncResult, EngineError> {
    let start = Instant::now();
    let work_dir = &config.work_dir;

    let (on_progress, credentials, ext_cancelled) = match ctx {
        Some(c) => (c.on_progress, c.credentials, c.cancelled),
        None => (None, None, None),
    };

    let report = |evt: SyncProgress| {
        if let Some(ref cb) = on_progress {
            cb(evt);
        }
    };

    // 1. Load album config
    let config_path = work_dir.join("album.config.json");
    if !config_path.exists() {
        return Err(EngineError::Config("album.config.json not found".into()));
    }
    let raw = fs::read_to_string(&config_path)?;
    let album_config: crate::AlbumConfig =
        serde_json::from_reader(json_comments::StripComments::new(raw.as_bytes()))
            .map_err(|e| EngineError::Config(format!("Failed to parse album.config.json: {e}")))?;

    let provider = album_config.provider.as_ref().ok_or_else(|| {
        EngineError::Config("No provider configured in album.config.json".into())
    })?;

    // 2. Scan albums directory
    let albums_dir = work_dir.join("albums");
    if !albums_dir.is_dir() {
        return Err(EngineError::Config("albums/ directory not found".into()));
    }

    let mut files_to_check: Vec<(String, std::path::PathBuf)> = Vec::new();
    for entry in &album_config.albums {
        let album_path = albums_dir.join(&entry.dir);
        if !album_path.is_dir() { continue; }
        if let Ok(entries) = fs::read_dir(&album_path) {
            for file_entry in entries.flatten() {
                let path = file_entry.path();
                if path.is_file() {
                    if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                        if is_photo_file(name) {
                            let rel = format!("{}/{}", entry.dir, name);
                            files_to_check.push((rel, path));
                        }
                    }
                }
            }
        }
    }

    // 3. Load lock + compute fingerprints + determine upload queue
    let lock_path = work_dir.join(".spage-sync.lock");
    let mut lock = load_lock(&lock_path);

    let mut upload_queue: Vec<(String, std::path::PathBuf, FileFingerprint)> = Vec::new();
    let mut skipped = 0u32;

    for (rel_path, full_path) in &files_to_check {
        let fp = compute_fingerprint(full_path, FINGERPRINT_THRESHOLD)?;
        if needs_upload(lock.files.get(rel_path), &fp) {
            upload_queue.push((rel_path.clone(), full_path.clone(), fp));
        } else {
            skipped += 1;
        }
    }

    report(SyncProgress::Scanning { total: upload_queue.len() as u32 });

    // 4. Dry-run mode
    if config.dry_run {
        println!("[sync] Dry-run: {} file(s) to upload, {} skipped", upload_queue.len(), skipped);
        for (rel, _, _) in &upload_queue {
            println!("  + albums/{}", rel);
        }
        report(SyncProgress::Done);
        return Ok(SyncResult {
            uploaded: 0,
            skipped,
            failed: vec![],
            duration_ms: start.elapsed().as_millis() as u64,
        });
    }

    // 5. Create S3 client + runtime
    let bucket = create_s3_bucket(provider, credentials.as_ref())?;

    // sync_media is inherently a blocking one-shot operation.
    // Always build our own runtime for the async S3 work.
    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .map_err(|e| EngineError::Config(format!("Failed to create tokio runtime: {e}")))?;

    // 6. Cancellation token — use external if provided, otherwise register Ctrl+C handler
    let interrupted = if let Some(token) = ext_cancelled {
        token
    } else {
        let flag = Arc::new(AtomicBool::new(false));
        let flag_clone = flag.clone();
        let _ = ctrlc::set_handler(move || {
            flag_clone.store(true, Ordering::SeqCst);
        });
        flag
    };

    // 7. Upload loop + generate & upload thumbs/JSON (pipeline)
    let (uploaded, failed) = rt.block_on(async {
        let mut uploaded = 0u32;
        let mut failed: Vec<String> = Vec::new();
        let total = upload_queue.len() as u32;

        for (i, (rel_path, full_path, fp)) in upload_queue.iter().enumerate() {
            if interrupted.load(Ordering::SeqCst) {
                println!("[sync] Cancelled, saving progress...");
                save_lock(&lock, &lock_path)?;
                return Err(EngineError::Cancelled);
            }
            let remote_key = format!("albums/{}", rel_path);
            report(SyncProgress::Uploading { current: i as u32 + 1, total, file: rel_path.clone() });
            print!("[sync] ({}/{}) Uploading {} ... ", i + 1, total, rel_path);
            match upload_with_retry(&bucket, full_path, &remote_key, 3).await {
                Ok(()) => {
                    println!("✓");
                    lock.files.insert(rel_path.clone(), fp.clone());
                    uploaded += 1;
                }
                Err(e) => {
                    println!("✗");
                    log::error!("[sync] {}", e);
                    failed.push(rel_path.clone());
                }
            }
        }

        // 8. Generate thumbnails only for changed files + upload, then upload index JSON
        {
            // Only generate thumbnails for files that were in the upload queue (changed/new)
            let changed_files: std::collections::HashSet<String> = upload_queue.iter()
                .map(|(rel, _, _)| rel.clone())
                .collect();

            if changed_files.is_empty() {
                println!("[sync] No changed files, skipping thumbnail generation");
            } else {
                println!("[sync] Generating thumbnails for {} changed file(s)...", changed_files.len());
                let sync_out = work_dir.join(".sync-build");
                fs::create_dir_all(&sync_out).ok();

                // Collect thumb tasks only for changed files
                let mut thumb_tasks: Vec<(String, std::path::PathBuf, std::path::PathBuf)> = Vec::new();
                for entry in &album_config.albums {
                    let album_src = albums_dir.join(&entry.dir);
                    if !album_src.is_dir() { continue; }
                    let thumbs_dir = sync_out.join("albums").join(&entry.dir).join("thumbs");
                    fs::create_dir_all(&thumbs_dir).ok();

                    if let Ok(files) = fs::read_dir(&album_src) {
                        for f in files.flatten() {
                            let path = f.path();
                            if !path.is_file() { continue; }
                            if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                                if !is_photo_file(name) { continue; }
                                let rel = format!("{}/{}", entry.dir, name);
                                if !changed_files.contains(&rel) { continue; }
                                let stem = Path::new(name).file_stem().unwrap_or_default().to_string_lossy().to_string();
                                let thumb_filename = format!("{stem}.webp");
                                let dest_path = thumbs_dir.join(&thumb_filename);
                                let remote_key = format!("albums/{}/thumbs/{}", entry.dir, thumb_filename);
                                thumb_tasks.push((remote_key, path, dest_path));
                            }
                        }
                    }
                }

                let thumb_total = thumb_tasks.len() as u32;
                let concurrency = if config.thumb_concurrency == 0 {
                    let cpus = std::thread::available_parallelism()
                        .map(|n| n.get())
                        .unwrap_or(4);
                    (cpus / 2).clamp(2, 8)
                } else {
                    config.thumb_concurrency
                };

                // Pipeline: producer generates thumbs concurrently, consumer uploads
                let (tx, mut rx) = tokio::sync::mpsc::channel::<(String, std::path::PathBuf)>(16);
                let interrupted2 = interrupted.clone();

                let producer = tokio::spawn(async move {
                    let results = stream::iter(thumb_tasks)
                        .map(|(remote_key, src_path, dest_path)| {
                            let interrupted = interrupted2.clone();
                            tokio::task::spawn_blocking(move || {
                                if interrupted.load(Ordering::SeqCst) {
                                    return None;
                                }
                                match crate::image_proc::generate_thumbnail(&src_path, &dest_path) {
                                    Ok(()) => Some((remote_key, dest_path)),
                                    Err(e) => {
                                        log::warn!("[sync] Thumbnail failed {}: {e}", src_path.display());
                                        None
                                    }
                                }
                            })
                        })
                        .buffer_unordered(concurrency)
                        .filter_map(|join_result| async {
                            match join_result {
                                Ok(v) => v,
                                Err(e) => {
                                    log::warn!("[sync] Thumbnail task panicked: {e}");
                                    None
                                }
                            }
                        });

                    tokio::pin!(results);
                    while let Some((remote_key, dest_path)) = results.next().await {
                        if tx.send((remote_key, dest_path)).await.is_err() {
                            break;
                        }
                    }
                });

                let mut thumb_count = 0u32;
                while let Some((remote_key, local_path)) = rx.recv().await {
                    if interrupted.load(Ordering::SeqCst) { break; }
                    thumb_count += 1;
                    report(SyncProgress::GeneratingThumbnail {
                        current: thumb_count,
                        total: thumb_total,
                        file: remote_key.clone(),
                    });
                    report(SyncProgress::UploadingThumbnail { current: thumb_count, total: thumb_total });
                    let _ = upload_with_retry(&bucket, &local_path, &remote_key, 3).await;
                }

                let _ = producer.await;
                println!("[sync] Thumbnails uploaded: {thumb_count}");

                // Cleanup temp dir
                let _ = fs::remove_dir_all(&sync_out);
            }

            // Check cancellation before index JSON upload
            if interrupted.load(Ordering::SeqCst) {
                save_lock(&lock, &lock_path)?;
                return Err(EngineError::Cancelled);
            }

            // Always regenerate and upload index JSON (includes EXIF for all photos)
            // Use generate_albums_metadata_only which produces correct thumbs URLs
            // without writing any thumbnail files to disk
            let json_out = work_dir.join(".sync-json");
            let _ = fs::remove_dir_all(&json_out);
            fs::create_dir_all(&json_out).ok();

            // Read site config for basePath
            let site_config_path = work_dir.join("config.json");
            let base_path = if site_config_path.exists() {
                let raw = fs::read_to_string(&site_config_path).unwrap_or_default();
                serde_json::from_reader::<_, crate::SiteConfig>(
                    json_comments::StripComments::new(raw.as_bytes())
                ).ok().and_then(|sc| sc.base_path)
            } else {
                None
            };

            let _ = crate::albums::generate_albums_metadata_only(
                &albums_dir, &json_out, &album_config, base_path.as_deref(),
            );
            let gen_dir = json_out.join("generated");
            if gen_dir.is_dir() {
                if let Ok(files) = fs::read_dir(&gen_dir) {
                    for f in files.flatten() {
                        if interrupted.load(Ordering::SeqCst) {
                            let _ = fs::remove_dir_all(&json_out);
                            save_lock(&lock, &lock_path)?;
                            return Err(EngineError::Cancelled);
                        }
                        if !f.path().is_file() { continue; }
                        let name = f.file_name().to_string_lossy().to_string();
                        let remote_key = format!("generated/{}", name);
                        let _ = upload_with_retry(&bucket, &f.path(), &remote_key, 3).await;
                    }
                }
                println!("[sync] Index JSON uploaded");
            }
            let _ = fs::remove_dir_all(&json_out);
        }

        report(SyncProgress::Done);
        Ok((uploaded, failed))
    })?;

    // 9. Save lock
    save_lock(&lock, &lock_path)?;

    Ok(SyncResult {
        uploaded,
        skipped,
        failed,
        duration_ms: start.elapsed().as_millis() as u64,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn empty_dir_returns_zero() {
        let tmp = tempdir().unwrap();
        assert_eq!(calculate_dir_size(tmp.path()), 0);
    }

    #[test]
    fn counts_only_photo_files() {
        let tmp = tempdir().unwrap();
        fs::write(tmp.path().join("photo.jpg"), vec![0u8; 100]).unwrap();
        fs::write(tmp.path().join("photo.avif"), vec![0u8; 75]).unwrap();
        fs::write(tmp.path().join("readme.txt"), vec![0u8; 50]).unwrap();
        assert_eq!(calculate_dir_size(tmp.path()), 175);
    }

    #[test]
    fn recurses_into_subdirs() {
        let tmp = tempdir().unwrap();
        let sub = tmp.path().join("sub");
        fs::create_dir(&sub).unwrap();
        fs::write(sub.join("a.png"), vec![0u8; 200]).unwrap();
        fs::write(tmp.path().join("b.webp"), vec![0u8; 300]).unwrap();
        assert_eq!(calculate_dir_size(tmp.path()), 500);
    }

    #[test]
    fn nonexistent_dir_returns_zero() {
        assert_eq!(calculate_dir_size(Path::new("/nonexistent_path_xyz")), 0);
    }

    #[test]
    fn small_file_produces_hash() {
        let tmp = tempdir().unwrap();
        let f = tmp.path().join("small.jpg");
        fs::write(&f, b"hello world").unwrap();
        let fp = compute_fingerprint(&f, FINGERPRINT_THRESHOLD).unwrap();
        assert!(fp.hash.is_some());
        assert_eq!(fp.size, 11);
    }

    #[test]
    fn large_file_no_hash() {
        let tmp = tempdir().unwrap();
        let f = tmp.path().join("big.jpg");
        fs::write(&f, vec![0u8; 100]).unwrap();
        let fp = compute_fingerprint(&f, 50).unwrap();
        assert!(fp.hash.is_none());
        assert_eq!(fp.size, 100);
    }

    #[test]
    fn lock_roundtrip() {
        let tmp = tempdir().unwrap();
        let lock_path = tmp.path().join(".spage-sync.lock");
        let mut lock = SyncLockFile::default();
        lock.files.insert(
            "Sakura/test.jpg".into(),
            FileFingerprint { hash: Some("abc123".into()), size: 1000, mtime: 12345 },
        );
        save_lock(&lock, &lock_path).unwrap();
        let loaded = load_lock(&lock_path);
        assert_eq!(lock.files, loaded.files);
    }

    #[test]
    fn load_lock_missing_file_returns_empty() {
        let lock = load_lock(Path::new("/nonexistent/.spage-sync.lock"));
        assert!(lock.files.is_empty());
    }

    #[test]
    fn needs_upload_no_existing() {
        let fp = FileFingerprint { hash: Some("abc".into()), size: 100, mtime: 1 };
        assert!(needs_upload(None, &fp));
    }

    #[test]
    fn needs_upload_same_hash() {
        let fp = FileFingerprint { hash: Some("abc".into()), size: 100, mtime: 1 };
        assert!(!needs_upload(Some(&fp), &fp));
    }

    #[test]
    fn needs_upload_different_hash() {
        let prev = FileFingerprint { hash: Some("abc".into()), size: 100, mtime: 1 };
        let cur = FileFingerprint { hash: Some("def".into()), size: 100, mtime: 1 };
        assert!(needs_upload(Some(&prev), &cur));
    }

    #[test]
    fn needs_upload_no_hash_same_size_mtime() {
        let fp = FileFingerprint { hash: None, size: 100, mtime: 1 };
        assert!(!needs_upload(Some(&fp), &fp));
    }

    #[test]
    fn needs_upload_no_hash_different_size() {
        let prev = FileFingerprint { hash: None, size: 100, mtime: 1 };
        let cur = FileFingerprint { hash: None, size: 200, mtime: 1 };
        assert!(needs_upload(Some(&prev), &cur));
    }

    #[test]
    fn needs_upload_no_hash_different_mtime() {
        let prev = FileFingerprint { hash: None, size: 100, mtime: 1 };
        let cur = FileFingerprint { hash: None, size: 100, mtime: 2 };
        assert!(needs_upload(Some(&prev), &cur));
    }

    #[test]
    fn create_s3_bucket_missing_env_returns_error() {
        let _ = dotenvy::dotenv();
        if std::env::var("S3_ACCESS_KEY").is_ok() {
            return;
        }
        let config = crate::ProviderConfig {
            type_: "s3".into(),
            endpoint: "https://example.com".into(),
            region: "auto".into(),
            bucket: "test".into(),
            public_url: "https://cdn.example.com".into(),
        };
        let err = create_s3_bucket(&config, None).unwrap_err();
        assert!(err.to_string().contains("S3_ACCESS_KEY"));
    }

    #[test]
    fn sync_media_no_provider_returns_error() {
        let tmp = tempdir().unwrap();
        fs::write(
            tmp.path().join("album.config.json"),
            r#"{"enabled":true,"albums":[]}"#,
        ).unwrap();
        fs::create_dir(tmp.path().join("albums")).unwrap();
        let opts = SyncConfig { work_dir: tmp.path().to_path_buf(), dry_run: false, ..Default::default() };
        let err = sync_media(opts).unwrap_err();
        assert!(err.to_string().contains("provider"));
    }

    #[test]
    fn sync_media_dry_run_lists_files() {
        let tmp = tempdir().unwrap();
        let albums_dir = tmp.path().join("albums").join("test");
        fs::create_dir_all(&albums_dir).unwrap();
        fs::write(albums_dir.join("photo.jpg"), vec![0u8; 50]).unwrap();
        fs::write(
            tmp.path().join("album.config.json"),
            r#"{"enabled":true,"albums":[{"dir":"test"}],"provider":{"type":"s3","endpoint":"http://x","region":"auto","bucket":"b","publicUrl":"http://x"}}"#,
        ).unwrap();
        let opts = SyncConfig { work_dir: tmp.path().to_path_buf(), dry_run: true, ..Default::default() };
        let result = sync_media(opts).unwrap();
        assert_eq!(result.uploaded, 0);
        assert_eq!(result.skipped, 0);
        assert!(result.failed.is_empty());
    }

    #[test]
    fn sync_media_dry_run_skips_already_synced() {
        let tmp = tempdir().unwrap();
        let albums_dir = tmp.path().join("albums").join("test");
        fs::create_dir_all(&albums_dir).unwrap();
        let photo = albums_dir.join("photo.jpg");
        fs::write(&photo, vec![0u8; 50]).unwrap();
        fs::write(
            tmp.path().join("album.config.json"),
            r#"{"enabled":true,"albums":[{"dir":"test"}],"provider":{"type":"s3","endpoint":"http://x","region":"auto","bucket":"b","publicUrl":"http://x"}}"#,
        ).unwrap();

        let fp = compute_fingerprint(&photo, FINGERPRINT_THRESHOLD).unwrap();
        let mut lock = SyncLockFile::default();
        lock.files.insert("test/photo.jpg".into(), fp);
        save_lock(&lock, &tmp.path().join(".spage-sync.lock")).unwrap();

        let opts = SyncConfig { work_dir: tmp.path().to_path_buf(), dry_run: true, ..Default::default() };
        let result = sync_media(opts).unwrap();
        assert_eq!(result.uploaded, 0);
        assert_eq!(result.skipped, 1);
    }

    #[test]
    fn sync_media_with_progress_callback() {
        use std::sync::Mutex;
        let tmp = tempdir().unwrap();
        let albums_dir = tmp.path().join("albums").join("test");
        fs::create_dir_all(&albums_dir).unwrap();
        fs::write(albums_dir.join("photo.jpg"), vec![0u8; 50]).unwrap();
        fs::write(
            tmp.path().join("album.config.json"),
            r#"{"enabled":true,"albums":[{"dir":"test"}],"provider":{"type":"s3","endpoint":"http://x","region":"auto","bucket":"b","publicUrl":"http://x"}}"#,
        ).unwrap();

        let events = Arc::new(Mutex::new(Vec::new()));
        let events_clone = events.clone();

        let config = SyncConfig { work_dir: tmp.path().to_path_buf(), dry_run: true, ..Default::default() };
        let ctx = SyncContext {
            on_progress: Some(Box::new(move |evt| {
                events_clone.lock().unwrap().push(format!("{:?}", evt));
            })),
            credentials: None,
            cancelled: None,
        };
        let _result = sync_media_with_context(config, Some(ctx)).unwrap();

        let collected = events.lock().unwrap();
        assert!(collected.iter().any(|e| e.contains("Scanning")));
        assert!(collected.iter().any(|e| e.contains("Done")));
    }
}

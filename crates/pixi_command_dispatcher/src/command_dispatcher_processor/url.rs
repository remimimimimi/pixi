use std::collections::hash_map::Entry;

use futures::FutureExt;
use url::Url;

use super::{CommandDispatcherProcessor, PendingUrlCheckout, TaskResult};
use crate::{
    Reporter, SourceCheckoutError,
    command_dispatcher::{UrlCheckoutTask, UrlDownloadResult},
};

impl CommandDispatcherProcessor {
    /// Called when a [`ForegroundMessage::UrlCheckout`] task was received.
    pub(crate) fn on_checkout_url(&mut self, task: UrlCheckoutTask) {
        let parent_context = task.parent.and_then(|ctx| self.reporter_context(ctx));
        let url_key = task.spec.url.clone();

        match self.url_checkouts.entry(url_key.clone()) {
            Entry::Occupied(mut existing_checkout) => match existing_checkout.get_mut() {
                PendingUrlCheckout::Pending(_, pending) => pending.push(task.tx),
                PendingUrlCheckout::CheckedOut(result) => {
                    let _ = task.tx.send(Ok(result.clone()));
                }
                PendingUrlCheckout::Errored => {
                    // Drop the sender, this will cause a cancellation on the other side.
                    drop(task.tx);
                }
            },
            Entry::Vacant(entry) => {
                // Notify the reporter that a new URL download has been queued.
                let reporter_id = self
                    .reporter
                    .as_deref_mut()
                    .and_then(Reporter::as_url_reporter)
                    .map(|reporter| reporter.on_queued(parent_context, &task.spec.url));

                entry.insert(PendingUrlCheckout::Pending(reporter_id, vec![task.tx]));

                // Notify the reporter that the download has started.
                if let Some((reporter, id)) = self
                    .reporter
                    .as_deref_mut()
                    .and_then(Reporter::as_url_reporter)
                    .zip(reporter_id)
                {
                    reporter.on_start(id)
                }

                let download_client = self.inner.download_client.clone();
                let cache_dir = self.inner.cache_dirs.source_archives().clone();
                let task_spec = task.spec.clone();

                self.pending_futures.push(
                    async move {
                        let result = download_and_extract_url(
                            &download_client,
                            &cache_dir,
                            task_spec.url,
                            task_spec.expected_md5,
                            task_spec.expected_sha256,
                        )
                        .await;
                        TaskResult::UrlCheckedOut(url_key, result)
                    }
                    .boxed_local(),
                );
            }
        }
    }

    /// Called when a URL download and extraction task has completed.
    pub(crate) fn on_url_checked_out(
        &mut self,
        url: Url,
        result: Result<UrlDownloadResult, SourceCheckoutError>,
    ) {
        let Some(PendingUrlCheckout::Pending(reporter_id, pending)) =
            self.url_checkouts.get_mut(&url)
        else {
            unreachable!("cannot get a result for a URL checkout that is not pending");
        };

        // Notify the reporter that the URL download has finished.
        if let Some((reporter, id)) = self
            .reporter
            .as_deref_mut()
            .and_then(Reporter::as_url_reporter)
            .zip(*reporter_id)
        {
            reporter.on_finished(id)
        }

        match result {
            Ok(download_result) => {
                for tx in pending.drain(..) {
                    let _ = tx.send(Ok(download_result.clone()));
                }

                // Store the result in the URL checkouts map.
                self.url_checkouts
                    .insert(url, PendingUrlCheckout::CheckedOut(download_result));
            }
            Err(mut err) => {
                // Only send the error to the first channel, drop the rest, which cancels them.
                for tx in pending.drain(..) {
                    match tx.send(Err(err)) {
                        Ok(_) => return,
                        Err(Err(failed_to_send)) => err = failed_to_send,
                        Err(Ok(_)) => unreachable!(),
                    }
                }

                self.url_checkouts.insert(url, PendingUrlCheckout::Errored);
            }
        }
    }
}

/// Downloads and extracts a URL archive.
async fn download_and_extract_url(
    download_client: &reqwest_middleware::ClientWithMiddleware,
    cache_dir: &std::path::Path,
    url: Url,
    expected_md5: Option<rattler_digest::Md5Hash>,
    expected_sha256: Option<rattler_digest::Sha256Hash>,
) -> Result<UrlDownloadResult, SourceCheckoutError> {
    use rattler_digest::{Md5, Md5Hash, Sha256, Sha256Hash, digest::Digest};
    use std::io::Write;
    use tempfile::NamedTempFile;

    // Download to temporary file
    let mut temp_file = NamedTempFile::new().map_err(SourceCheckoutError::IoError)?;
    let response = download_client
        .get(url.as_str())
        .send()
        .await
        .map_err(|e| SourceCheckoutError::NetworkError(e.to_string()))?;

    if !response.status().is_success() {
        return Err(SourceCheckoutError::NetworkError(format!(
            "HTTP {} downloading {}",
            response.status(),
            url
        )));
    }

    let mut md5_hasher = Md5::new();
    let mut sha256_hasher = Sha256::new();

    let mut response = response;
    while let Some(chunk) = response
        .chunk()
        .await
        .map_err(|e| SourceCheckoutError::NetworkError(e.to_string()))?
    {
        temp_file
            .write_all(&chunk)
            .map_err(SourceCheckoutError::IoError)?;
        md5_hasher.update(&chunk);
        sha256_hasher.update(&chunk);
    }

    // Finalize hashes
    let computed_md5 = Md5Hash::from(md5_hasher.finalize());
    let computed_sha256 = Sha256Hash::from(sha256_hasher.finalize());

    // Verify checksums if provided
    if let Some(expected_md5) = expected_md5 {
        if computed_md5 != expected_md5 {
            return Err(SourceCheckoutError::ChecksumMismatch(format!(
                "MD5 mismatch: expected {:x}, got {:x}",
                expected_md5, computed_md5
            )));
        }
    }

    if let Some(expected_sha256) = expected_sha256 {
        if computed_sha256 != expected_sha256 {
            return Err(SourceCheckoutError::ChecksumMismatch(format!(
                "SHA256 mismatch: expected {:x}, got {:x}",
                expected_sha256, computed_sha256
            )));
        }
    }

    // Create cache key from SHA256 hash
    let cache_key = format!("url-{:x}", computed_sha256);
    let extract_path = cache_dir.join(cache_key);

    // Extract to cache directory if not already cached
    if !extract_path.exists() {
        fs_err::create_dir_all(&extract_path).map_err(SourceCheckoutError::IoError)?;

        let temp_path = temp_file.into_temp_path().keep().map_err(|e| {
            SourceCheckoutError::ArchiveError(format!("Failed to keep temp file: {}", e))
        })?;

        extract_archive(&temp_path, &extract_path).await?;

        // Clean up temporary file
        let _ = fs_err::remove_file(&temp_path);
    }

    Ok(UrlDownloadResult {
        path: extract_path,
        computed_sha256,
    })
}

/// Extracts an archive to the specified directory.
async fn extract_archive(
    archive_path: &std::path::Path,
    extract_to: &std::path::Path,
) -> Result<(), SourceCheckoutError> {
    use flate2::read::GzDecoder;
    use std::fs::File;
    use tar::Archive;

    let file = File::open(archive_path).map_err(SourceCheckoutError::IoError)?;

    // Try to detect archive format from file extension or URL
    let archive_name = archive_path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("");

    if archive_name.ends_with(".tar.gz") || archive_name.ends_with(".tgz") {
        let decoder = GzDecoder::new(file);
        let mut archive = Archive::new(decoder);
        archive
            .unpack(extract_to)
            .map_err(SourceCheckoutError::IoError)?;
    } else if archive_name.ends_with(".tar") {
        let mut archive = Archive::new(file);
        archive
            .unpack(extract_to)
            .map_err(SourceCheckoutError::IoError)?;
    } else if archive_name.ends_with(".zip") {
        let mut archive = zip::ZipArchive::new(file).map_err(|e| {
            SourceCheckoutError::ArchiveError(format!("Failed to open ZIP archive: {}", e))
        })?;
        archive.extract(extract_to).map_err(|e| {
            SourceCheckoutError::ArchiveError(format!("Failed to extract ZIP archive: {}", e))
        })?;
    } else {
        return Err(SourceCheckoutError::UnsupportedArchiveFormat(
            archive_name.to_string(),
        ));
    }

    Ok(())
}

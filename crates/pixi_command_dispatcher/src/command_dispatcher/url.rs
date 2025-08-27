use std::{
    io::Write,
    path::{Path, PathBuf},
};

use flate2::read::GzDecoder;
use pixi_record::{PinnedSourceSpec, PinnedUrlSpec};
use pixi_spec::UrlSourceSpec;
use rattler_digest::{Md5Hash, Sha256Hash};
use tar::Archive;
use tempfile::NamedTempFile;
use url::Url;

use crate::{CommandDispatcher, CommandDispatcherError, SourceCheckout, SourceCheckoutError};

impl CommandDispatcher {
    /// Downloads and pins a URL source specification.
    pub async fn pin_and_checkout_url(
        &self,
        url_spec: UrlSourceSpec,
    ) -> Result<SourceCheckout, CommandDispatcherError<SourceCheckoutError>> {
        // Download and verify the archive
        let (temp_path, computed_sha256) = self
            .download_and_verify_url(&url_spec.url, url_spec.md5, url_spec.sha256)
            .await
            .map_err(CommandDispatcherError::Failed)?;

        // Create cache key from SHA256 hash
        let cache_key = format!("url-{:x}", computed_sha256);
        let extract_path = self.cache_dirs().source_archives().join(cache_key);

        // Extract to cache directory if not already cached
        if !extract_path.exists() {
            fs_err::create_dir_all(&extract_path)
                .map_err(SourceCheckoutError::IoError)
                .map_err(CommandDispatcherError::Failed)?;
            self.extract_archive(&temp_path, &extract_path)
                .await
                .map_err(CommandDispatcherError::Failed)?;
        }

        // Clean up temporary file
        let _ = fs_err::remove_file(&temp_path);

        let pinned = PinnedUrlSpec {
            url: url_spec.url,
            sha256: computed_sha256,
            md5: url_spec.md5,
        };

        Ok(SourceCheckout {
            path: extract_path,
            pinned: PinnedSourceSpec::Url(pinned),
        })
    }

    /// Checks out a pinned URL source.
    pub async fn checkout_pinned_url(
        &self,
        pinned_spec: PinnedUrlSpec,
    ) -> Result<SourceCheckout, CommandDispatcherError<SourceCheckoutError>> {
        let cache_key = format!("url-{:x}", pinned_spec.sha256);
        let extract_path = self.cache_dirs().source_archives().join(cache_key);

        if !extract_path.exists() {
            // Re-download if cache is missing
            let url_spec = UrlSourceSpec::from(pinned_spec.clone());
            return self.pin_and_checkout_url(url_spec).await;
        }

        Ok(SourceCheckout {
            path: extract_path,
            pinned: PinnedSourceSpec::Url(pinned_spec),
        })
    }

    /// Downloads a URL and verifies its checksum.
    async fn download_and_verify_url(
        &self,
        url: &Url,
        expected_md5: Option<Md5Hash>,
        expected_sha256: Option<Sha256Hash>,
    ) -> Result<(PathBuf, Sha256Hash), SourceCheckoutError> {
        use rattler_digest::digest::Digest;
        use rattler_digest::{Md5, Sha256};

        // Download to temporary file
        let mut temp_file = NamedTempFile::new().map_err(SourceCheckoutError::IoError)?;
        let response = self
            .download_client()
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

        // Convert temp file to permanent path
        let temp_path = temp_file.into_temp_path().keep().map_err(|e| {
            SourceCheckoutError::ArchiveError(format!("Failed to keep temp file: {}", e))
        })?;

        Ok((temp_path, computed_sha256))
    }

    /// Extracts an archive to the specified directory.
    async fn extract_archive(
        &self,
        archive_path: &Path,
        extract_to: &Path,
    ) -> Result<(), SourceCheckoutError> {
        use std::fs::File;

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
}

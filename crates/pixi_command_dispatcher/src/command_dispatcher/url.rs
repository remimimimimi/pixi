use std::path::PathBuf;

use pixi_record::{PinnedSourceSpec, PinnedUrlSpec};
use pixi_spec::UrlSourceSpec;
use rattler_digest::{Md5Hash, Sha256Hash};
use url::Url;

use super::{Task, TaskSpec};
use crate::{CommandDispatcher, CommandDispatcherError, SourceCheckout, SourceCheckoutError};

/// A task that downloads and extracts a URL archive.
#[derive(Debug, Clone)]
pub struct UrlDownloadTask {
    pub url: Url,
    pub expected_md5: Option<Md5Hash>,
    pub expected_sha256: Option<Sha256Hash>,
}

/// The result of downloading and extracting a URL.
#[derive(Debug, Clone)]
pub struct UrlDownloadResult {
    pub path: PathBuf,
    pub computed_sha256: Sha256Hash,
}

/// A task that is sent to the background to download and extract a URL.
pub(crate) type UrlCheckoutTask = Task<UrlDownloadTask>;

impl TaskSpec for UrlDownloadTask {
    type Output = UrlDownloadResult;
    type Error = SourceCheckoutError;
}

impl CommandDispatcher {
    /// Downloads and pins a URL source specification.
    pub async fn pin_and_checkout_url(
        &self,
        url_spec: UrlSourceSpec,
    ) -> Result<SourceCheckout, CommandDispatcherError<SourceCheckoutError>> {
        // Create cache key from SHA256 hash if available, or download to get it
        let cache_key = if let Some(sha256) = url_spec.sha256 {
            format!("url-{:x}", sha256)
        } else {
            // We need to download to get the SHA256 hash
            let result = self
                .checkout_url(UrlDownloadTask {
                    url: url_spec.url.clone(),
                    expected_md5: url_spec.md5,
                    expected_sha256: url_spec.sha256,
                })
                .await?;

            let pinned = PinnedUrlSpec {
                url: url_spec.url,
                sha256: result.computed_sha256,
                md5: url_spec.md5,
            };

            return Ok(SourceCheckout {
                path: result.path,
                pinned: PinnedSourceSpec::Url(pinned),
            });
        };

        let extract_path = self.cache_dirs().source_archives().join(cache_key);

        if extract_path.exists() {
            // Cache hit - return existing extraction
            let pinned = PinnedUrlSpec {
                url: url_spec.url,
                sha256: url_spec.sha256.expect("SHA256 should be available"),
                md5: url_spec.md5,
            };

            Ok(SourceCheckout {
                path: extract_path,
                pinned: PinnedSourceSpec::Url(pinned),
            })
        } else {
            // Cache miss - download and extract in background
            let result = self
                .checkout_url(UrlDownloadTask {
                    url: url_spec.url.clone(),
                    expected_md5: url_spec.md5,
                    expected_sha256: url_spec.sha256,
                })
                .await?;

            let pinned = PinnedUrlSpec {
                url: url_spec.url,
                sha256: result.computed_sha256,
                md5: url_spec.md5,
            };

            Ok(SourceCheckout {
                path: result.path,
                pinned: PinnedSourceSpec::Url(pinned),
            })
        }
    }

    /// Checks out a pinned URL source.
    pub async fn checkout_pinned_url(
        &self,
        pinned_spec: PinnedUrlSpec,
    ) -> Result<SourceCheckout, CommandDispatcherError<SourceCheckoutError>> {
        let cache_key = format!("url-{:x}", pinned_spec.sha256);
        let extract_path = self.cache_dirs().source_archives().join(cache_key);

        if !extract_path.exists() {
            // Re-download if cache is missing - use background task
            let result = self
                .checkout_url(UrlDownloadTask {
                    url: pinned_spec.url.clone(),
                    expected_md5: pinned_spec.md5,
                    expected_sha256: Some(pinned_spec.sha256),
                })
                .await?;

            Ok(SourceCheckout {
                path: result.path,
                pinned: PinnedSourceSpec::Url(pinned_spec),
            })
        } else {
            Ok(SourceCheckout {
                path: extract_path,
                pinned: PinnedSourceSpec::Url(pinned_spec),
            })
        }
    }

    /// Downloads and extracts a URL using background task execution.
    pub async fn checkout_url(
        &self,
        task: UrlDownloadTask,
    ) -> Result<UrlDownloadResult, CommandDispatcherError<SourceCheckoutError>> {
        self.execute_task(task).await
    }
}

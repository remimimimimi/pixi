use std::{collections::HashMap, time::Duration};

use indexmap::IndexMap;
use indicatif::{MultiProgress, ProgressBar};
use pixi_command_dispatcher::{ReporterContext, reporter::UrlCheckoutId};
use url::Url;

/// A reporter implementation for URL downloads.
pub struct UrlCheckoutProgress {
    /// The multi-progress bar. Usually, this is the global multi-progress bar.
    multi_progress: MultiProgress,
    /// The progress bar that is used as an anchor for placing other progress.
    anchor: ProgressBar,
    /// The id of the next checkout
    next_id: usize,
    /// A map of progress bars, by ID.
    bars: IndexMap<UrlCheckoutId, ProgressBar>,
    /// References to the URL info
    urls: HashMap<UrlCheckoutId, Url>,
}

impl UrlCheckoutProgress {
    /// Creates a new URL checkout reporter.
    pub fn new(multi_progress: MultiProgress, anchor: ProgressBar) -> Self {
        Self {
            multi_progress,
            anchor,
            next_id: 0,
            bars: Default::default(),
            urls: Default::default(),
        }
    }

    /// Returns a unique ID for a new progress bar.
    fn next_checkout_id(&mut self) -> UrlCheckoutId {
        let id = UrlCheckoutId(self.next_id);
        self.next_id += 1;
        id
    }

    /// Similar to the default pixi_progress::default_progress_style, but with a
    /// spinner in front.
    pub fn spinner_style() -> indicatif::ProgressStyle {
        indicatif::ProgressStyle::with_template("  {spinner:.green} {prefix:30!} {wide_msg:.dim}")
            .expect("should be able to create a progress bar style")
    }

    /// Returns the URL for the given checkout ID.
    pub fn url(&self, id: UrlCheckoutId) -> &Url {
        self.urls
            .get(&id)
            .expect("the progress bar needs to be inserted for this checkout")
    }

    /// Returns the progress bar at the bottom
    pub fn last_progress_bar(&self) -> Option<&ProgressBar> {
        self.bars.last().map(|(_, pb)| pb)
    }
}

impl pixi_command_dispatcher::UrlCheckoutReporter for UrlCheckoutProgress {
    /// Called when a URL download was queued on the [`CommandDispatcher`].
    fn on_queued(&mut self, _context: Option<ReporterContext>, url: &Url) -> UrlCheckoutId {
        let checkout_id = self.next_checkout_id();
        self.urls.insert(checkout_id, url.clone());
        checkout_id
    }

    fn on_start(&mut self, checkout_id: UrlCheckoutId) {
        let pb = self.multi_progress.insert_after(
            self.last_progress_bar().unwrap_or(&self.anchor),
            ProgressBar::hidden(),
        );
        let url = self.url(checkout_id);
        pb.set_style(UrlCheckoutProgress::spinner_style());
        pb.set_prefix("downloading archives");
        pb.set_message(format!("downloading {}", url));
        pb.enable_steady_tick(Duration::from_millis(100));

        self.bars.insert(checkout_id, pb);
    }

    fn on_finished(&mut self, checkout_id: UrlCheckoutId) {
        let removed_pb = self
            .bars
            .shift_remove(&checkout_id)
            .expect("the progress bar needs to be inserted for this checkout");
        let url = self.url(checkout_id);
        removed_pb.finish_with_message(format!("download complete {}", url));
        removed_pb.finish_and_clear();
    }
}

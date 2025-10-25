use std::num::NonZero;

/// Defines concurrency and resource limits for the dispatcher.
#[derive(Debug, Clone, Copy, Default)]
pub struct Limits {
    /// The maximum number of concurrent "solve" style tasks.
    pub max_concurrent_solves: Limit,

    /// The maximum number of concurrent "build" style tasks.
    pub max_concurrent_builds: Limit,
}

/// Defines the type of limit to apply.
#[derive(Debug, Clone, Copy, Default)]
pub enum Limit {
    /// There is no limit.
    None,

    /// There is an upper limit.
    Max(NonZero<usize>),

    /// Use a heuristic to determine the limit.
    #[default]
    Default,
}

impl From<usize> for Limit {
    fn from(value: usize) -> Self {
        NonZero::new(value).map(Limit::Max).unwrap_or(Limit::None)
    }
}

#[derive(Debug, Clone, Copy)]
pub struct ResolvedLimits {
    /// The maximum number of concurrent solve operations that can be performed.
    pub max_concurrent_solves: Option<usize>,

    /// The maximum number of concurrent builds that can be performed.
    pub max_concurrent_builds: Option<usize>,
}

impl From<Limits> for ResolvedLimits {
    fn from(value: Limits) -> Self {
        let max_concurrent_solves = match value.max_concurrent_solves {
            Limit::None => None,
            Limit::Max(max) => Some(max.get()),
            Limit::Default => Some(
                std::thread::available_parallelism()
                    .map(NonZero::get)
                    .unwrap_or(1),
            ),
        };

        let max_concurrent_builds = match value.max_concurrent_builds {
            Limit::None => None,
            Limit::Max(max) => Some(max.get()),
            Limit::Default => Some(1), // Default to 1 build at a time.
        };

        ResolvedLimits {
            max_concurrent_solves,
            max_concurrent_builds,
        }
    }
}

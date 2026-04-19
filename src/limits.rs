//! Search limits — depth, time, node count. All optional; absent fields mean "no limit".

use std::time::Duration;

pub(crate) const MAX_SEARCH_DEPTH: u8 = 64;

#[derive(Copy, Clone, Debug)]
pub struct Limits {
    pub(crate) max_depth: u8,
    pub(crate) max_time: Option<Duration>,
    pub(crate) max_nodes: Option<u64>,
}

impl Default for Limits {
    fn default() -> Self {
        Limits { max_depth: MAX_SEARCH_DEPTH, max_time: Some(Duration::from_millis(1000)), max_nodes: None }
    }
}

impl Limits {
    pub const fn new() -> Self { Limits { max_depth: MAX_SEARCH_DEPTH, max_time: None, max_nodes: None } }

    #[must_use]
    pub const fn depth(mut self, d: u8) -> Self {
        self.max_depth = d;
        self
    }

    #[must_use]
    pub const fn time(mut self, d: Duration) -> Self {
        self.max_time = Some(d);
        self
    }
}

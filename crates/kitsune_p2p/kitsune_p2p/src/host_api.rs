use must_future::MustBoxFuture;
use std::sync::Arc;

use kitsune_p2p_types::{bin_types::KitsuneSpace, dht_arc::DhtArcSet};

use crate::event::MetricRecord;

/// A boxed future result with dynamic error type
pub type KitsuneHostResult<'a, T> =
    MustBoxFuture<'a, Result<T, Box<dyn 'a + Send + Sync + std::error::Error>>>;

/// The interface to be implemented by the host, which handles various requests
/// for data
pub trait KitsuneHost {
    /// Extrapolated Peer Coverage
    fn peer_extrapolated_coverage(
        &self,
        space: Arc<KitsuneSpace>,
        dht_arc_set: DhtArcSet,
    ) -> KitsuneHostResult<Vec<f64>>;

    /// Record a set of metric records
    fn record_metrics(
        &self,
        space: Arc<KitsuneSpace>,
        records: Vec<MetricRecord>,
    ) -> KitsuneHostResult<()>;
}

/// Trait object for the host interface
pub type HostApi = std::sync::Arc<dyn KitsuneHost + Send + Sync>;

// Test-only stub which mostly panics
#[cfg(any(test, feature = "test_utils"))]
mod host_stub;
#[cfg(any(test, feature = "test_utils"))]
pub use host_stub::*;

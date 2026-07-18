//! Host callback cassette and replay support.

use sim_kernel::Result;
use sim_lib_stream_core::{
    PushResult, StreamCassette, StreamItem, StreamMetadata, StreamPacket, StreamStats,
    TransportProfile,
};

use crate::HostCallbackQueue;

/// Deterministic recording of host callback items.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct HostCallbackCassette {
    items: Vec<StreamItem>,
}

/// Outcome counts from deterministic host callback cassette replay.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct HostCallbackReplayReport {
    /// Callback items accepted by the target queue.
    pub accepted: u64,
    /// Callback items dropped by the target queue's drop-newest policy.
    pub dropped_newest: u64,
    /// Callback items evicted from the target queue's drop-oldest policy.
    pub dropped_oldest: u64,
    /// Callback items rejected by the target queue's overflow policy.
    pub rejected: u64,
    /// Callback items refused because the target queue was closed.
    pub closed: u64,
}

impl HostCallbackCassette {
    /// Creates an empty cassette.
    pub fn new() -> Self {
        Self::default()
    }

    /// Creates a cassette from callback items.
    pub fn from_items(items: Vec<StreamItem>) -> Self {
        Self { items }
    }

    /// Appends one callback item.
    pub fn record_item(&mut self, item: StreamItem) {
        self.items.push(item);
    }

    /// Appends one packet with no explicit ticks.
    pub fn record_packet(&mut self, packet: StreamPacket) {
        self.record_item(StreamItem::new(packet));
    }

    /// Returns recorded callback items.
    pub fn items(&self) -> &[StreamItem] {
        &self.items
    }

    /// Converts a shared stream cassette into a callback cassette.
    pub fn from_stream_cassette(cassette: &StreamCassette) -> Result<Self> {
        Ok(Self {
            items: cassette.items()?,
        })
    }

    /// Converts recorded callbacks into the shared stream cassette format.
    pub fn to_stream_cassette(
        &self,
        metadata: StreamMetadata,
        profile: TransportProfile,
    ) -> Result<StreamCassette> {
        StreamCassette::from_items(
            metadata,
            self.items.clone(),
            profile,
            StreamStats {
                pushed: self.items.len() as u64,
                accepted: self.items.len() as u64,
                ..StreamStats::default()
            },
        )
    }

    /// Replays every recorded item into a host callback queue.
    pub fn replay(&self, queue: &HostCallbackQueue) -> Result<HostCallbackReplayReport> {
        let mut report = HostCallbackReplayReport::default();
        for item in &self.items {
            report.record(queue.callback_item(item.clone())?);
        }
        Ok(report)
    }
}

impl HostCallbackReplayReport {
    fn record(&mut self, result: PushResult) {
        match result {
            PushResult::Accepted => self.accepted = self.accepted.saturating_add(1),
            PushResult::DroppedNewest(_) => {
                self.dropped_newest = self.dropped_newest.saturating_add(1);
            }
            PushResult::DroppedOldest(_) => {
                self.dropped_oldest = self.dropped_oldest.saturating_add(1);
            }
            PushResult::Rejected(_) => self.rejected = self.rejected.saturating_add(1),
            PushResult::Closed(_) => self.closed = self.closed.saturating_add(1),
        }
    }
}

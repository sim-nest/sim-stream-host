//! Host callback cassette and replay support.

use sim_kernel::Result;
use sim_lib_stream_core::{
    StreamCassette, StreamItem, StreamMetadata, StreamPacket, StreamStats, TransportProfile,
};

use crate::HostCallbackQueue;

/// Deterministic recording of host callback items.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct HostCallbackCassette {
    items: Vec<StreamItem>,
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
    pub fn replay(&self, queue: &HostCallbackQueue) -> Result<()> {
        for item in &self.items {
            queue.callback_item(item.clone())?;
        }
        Ok(())
    }
}

//! Cloneable bounded queue handle passed to host callbacks.

use std::sync::Arc;

use sim_kernel::{Error, Result, Symbol};
use sim_lib_stream_core::{
    PushResult, StreamInspectorSnapshot, StreamItem, StreamMedia, StreamPacket, StreamStats,
    StreamValue, TransportProfile,
};

/// Cloneable handle host callbacks use to enqueue packets into a stream.
///
/// Wraps a push [`StreamValue`] and enforces that every enqueued packet matches
/// the device media. The handle is cheap to clone and callbacks enqueue through
/// non-blocking calls.
#[derive(Clone)]
pub struct HostCallbackQueue {
    media: StreamMedia,
    stream: Arc<StreamValue>,
}

impl HostCallbackQueue {
    /// Wraps a push stream, capturing its media for callback validation.
    pub fn new(stream: Arc<StreamValue>) -> Self {
        Self {
            media: stream.metadata().media(),
            stream,
        }
    }

    /// Returns a shared handle to the underlying stream value.
    pub fn stream(&self) -> Arc<StreamValue> {
        Arc::clone(&self.stream)
    }

    /// Enqueues a packet from a host callback after checking its media.
    pub fn callback_packet(&self, packet: StreamPacket) -> Result<PushResult> {
        self.ensure_media(&packet)?;
        self.stream.push_packet(StreamItem::new(packet))
    }

    /// Enqueues a prebuilt stream item from a host callback after checking its
    /// media.
    pub fn callback_item(&self, item: StreamItem) -> Result<PushResult> {
        self.ensure_media(item.packet())?;
        self.stream.push_packet(item)
    }

    /// Removes up to `limit` buffered items from the stream.
    pub fn drain(&self, limit: usize) -> Result<Vec<StreamItem>> {
        self.stream.take_packets(limit)
    }

    /// Closes the push side of the stream.
    pub fn close(&self) -> Result<()> {
        self.stream.close_push()
    }

    /// Cancels the stream and drops buffered packets.
    pub fn cancel(&self) -> Result<()> {
        self.stream.cancel()
    }

    /// Returns the current stream statistics.
    pub fn stats(&self) -> Result<StreamStats> {
        self.stream.stats()
    }

    /// Builds an inspector snapshot for the stream on the given route and
    /// transport profile.
    pub fn inspector(
        &self,
        route: Symbol,
        profile: &TransportProfile,
        recent_diagnostics: Vec<Symbol>,
    ) -> Result<StreamInspectorSnapshot> {
        StreamInspectorSnapshot::from_stream_value(&self.stream, route, profile, recent_diagnostics)
    }

    fn ensure_media(&self, packet: &StreamPacket) -> Result<()> {
        let packet_media = match packet {
            StreamPacket::Pcm(_) => StreamMedia::Pcm,
            StreamPacket::Midi(_) => StreamMedia::Midi,
            StreamPacket::Diagnostic(_) => StreamMedia::Diagnostic,
            StreamPacket::Data(_) => StreamMedia::Data,
        };
        if packet_media == self.media {
            Ok(())
        } else {
            Err(Error::TypeMismatch {
                expected: "host callback packet matching device media",
                found: "host callback packet for another media",
            })
        }
    }
}

use std::sync::Arc;

use sim_kernel::{Cx, DefaultFactory, EagerPolicy, Symbol};
use sim_lib_stream_core::{BufferPolicy, ClockDomain, MidiPacket, MidiPacketEvent, StreamMedia};
pub(super) use sim_value::access::field;

use crate::{
    HostClockInfo, HostDirection, HostLatencyInfo, HostStreamConfig, HostStreamConfigRequest,
    fake_backend_symbol, rtp_midi_backend_symbol, stream_host_capability,
};

pub(super) fn test_cx() -> Cx {
    Cx::new(Arc::new(EagerPolicy), Arc::new(DefaultFactory))
}

pub(super) fn authorized_cx() -> Cx {
    let mut cx = test_cx();
    cx.grant(stream_host_capability());
    cx
}

pub(super) fn recorded_effect_kinds(cx: &Cx) -> Vec<Symbol> {
    cx.effect_ledger()
        .records()
        .iter()
        .filter_map(|record| cx.effect_ledger().effect(&record.effect))
        .map(|effect| effect.kind.clone())
        .collect()
}

pub(super) fn realtime_audio_config(
    media: StreamMedia,
    clock_domain: ClockDomain,
) -> HostStreamConfig {
    let clock = clock_domain.symbol();
    let request = HostStreamConfigRequest::new(
        fake_backend_symbol(),
        Symbol::new("fake/pcm"),
        media,
        HostDirection::Output,
        BufferPolicy::bounded(4).unwrap(),
    )
    .with_clock(clock.clone());
    HostStreamConfig::from_request(
        request,
        HostLatencyInfo::default(),
        HostClockInfo::new(clock, Some(48_000), true),
    )
}

pub(super) fn lan_midi_config(media: StreamMedia, clock_domain: ClockDomain) -> HostStreamConfig {
    let clock = clock_domain.symbol();
    let request = HostStreamConfigRequest::new(
        rtp_midi_backend_symbol(),
        Symbol::new("rtp-midi/config"),
        media,
        HostDirection::Input,
        BufferPolicy::bounded(4).unwrap(),
    )
    .with_clock(clock.clone());
    HostStreamConfig::from_request(
        request,
        HostLatencyInfo::default(),
        HostClockInfo::new(clock, None, true),
    )
}

pub(super) fn note_packet(ticks: i64) -> MidiPacket {
    MidiPacket::new(vec![
        MidiPacketEvent::new(ticks, 480, vec![0x90, 60, 100]).unwrap(),
    ])
    .unwrap()
}

use std::{
    rc::Rc,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
};

use sim_lib_stream_core::{ClockDomain, StreamMedia};

use crate::{HostOpenStream, HostStreamDriver};

use super::support::{lan_midi_config, realtime_audio_config};

#[test]
fn realtime_local_audio_opening_rejects_wrong_media_and_clock() {
    let config = realtime_audio_config(StreamMedia::Pcm, ClockDomain::Sample);
    let opened = HostOpenStream::new_realtime_local_audio(config).unwrap();
    assert_eq!(opened.config().media(), StreamMedia::Pcm);

    let err = match HostOpenStream::new_realtime_local_audio(realtime_audio_config(
        StreamMedia::Data,
        ClockDomain::Sample,
    )) {
        Ok(_) => panic!("data stream should not open as realtime audio"),
        Err(err) => err,
    };
    assert!(err.to_string().contains("PCM"));

    let err = match HostOpenStream::new_realtime_local_audio(realtime_audio_config(
        StreamMedia::Pcm,
        ClockDomain::Wall,
    )) {
        Ok(_) => panic!("wall-clock stream should not open as realtime audio"),
        Err(err) => err,
    };
    assert!(err.to_string().contains("sample clock"));
}

#[test]
fn realtime_local_audio_close_shutdowns_attached_driver() {
    let shutdowns = Arc::new(AtomicUsize::new(0));
    let driver: Rc<dyn HostStreamDriver> = Rc::new(CountingDriver {
        shutdowns: Arc::clone(&shutdowns),
    });
    let opened = HostOpenStream::new_realtime_local_audio_with_driver(
        realtime_audio_config(StreamMedia::Pcm, ClockDomain::Sample),
        driver,
    )
    .unwrap();

    opened.close().unwrap();

    assert_eq!(shutdowns.load(Ordering::SeqCst), 1);
}

#[test]
fn realtime_local_audio_clone_close_close_shutdowns_driver_once() {
    let shutdowns = Arc::new(AtomicUsize::new(0));
    let driver: Rc<dyn HostStreamDriver> = Rc::new(CountingDriver {
        shutdowns: Arc::clone(&shutdowns),
    });
    let opened = HostOpenStream::new_realtime_local_audio_with_driver(
        realtime_audio_config(StreamMedia::Pcm, ClockDomain::Sample),
        driver,
    )
    .unwrap();
    let cloned = opened.clone();

    opened.close().unwrap();
    cloned.close().unwrap();

    assert_eq!(shutdowns.load(Ordering::SeqCst), 1);
}

#[test]
fn realtime_local_audio_close_then_cancel_shutdowns_driver_once() {
    let shutdowns = Arc::new(AtomicUsize::new(0));
    let driver: Rc<dyn HostStreamDriver> = Rc::new(CountingDriver {
        shutdowns: Arc::clone(&shutdowns),
    });
    let opened = HostOpenStream::new_realtime_local_audio_with_driver(
        realtime_audio_config(StreamMedia::Pcm, ClockDomain::Sample),
        driver,
    )
    .unwrap();

    opened.close().unwrap();
    opened.cancel().unwrap();

    assert_eq!(shutdowns.load(Ordering::SeqCst), 1);
}

#[test]
fn lan_midi_control_opening_rejects_wrong_media_and_clock() {
    let opened = HostOpenStream::new_lan_midi_control(lan_midi_config(
        StreamMedia::Midi,
        ClockDomain::MidiTick,
    ))
    .unwrap();
    assert_eq!(opened.config().media(), StreamMedia::Midi);

    let err = match HostOpenStream::new_lan_midi_control(lan_midi_config(
        StreamMedia::Pcm,
        ClockDomain::MidiTick,
    )) {
        Ok(_) => panic!("PCM stream should not open as LAN MIDI/control"),
        Err(err) => err,
    };
    assert!(err.to_string().contains("MIDI media"));

    let err = match HostOpenStream::new_lan_midi_control(lan_midi_config(
        StreamMedia::Midi,
        ClockDomain::Wall,
    )) {
        Ok(_) => panic!("wall-clock stream should not open as LAN MIDI/control"),
        Err(err) => err,
    };
    assert!(err.to_string().contains("MIDI tick or control clock"));
}

struct CountingDriver {
    shutdowns: Arc<AtomicUsize>,
}

impl HostStreamDriver for CountingDriver {
    fn shutdown(&self) -> sim_kernel::Result<()> {
        self.shutdowns.fetch_add(1, Ordering::SeqCst);
        Ok(())
    }
}

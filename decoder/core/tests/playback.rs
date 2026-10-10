// SPDX-License-Identifier: AGPL-3.0-or-later
use std::io;
use std::sync::Arc;

use tinystream_decoder_core::sequence::{Event, Item, Sequencer};
use tinystream_decoder_core::{RangeSource, Track};
struct Bytes(Vec<u8>);
impl RangeSource for Bytes {
    fn len(&self) -> io::Result<u64> {
        Ok(self.0.len() as u64)
    }

    fn read_range(&self, start: u64, end: u64) -> io::Result<Vec<u8>> {
        Ok(self.0[start as usize..=end as usize].to_vec())
    }
}
fn wav(frames: u32, rate: u32, sample: i16) -> Arc<dyn RangeSource> {
    let mut b = Vec::new();
    b.extend(b"RIFF");
    b.extend((36 + frames * 4).to_le_bytes());
    b.extend(b"WAVEfmt ");
    b.extend(16u32.to_le_bytes());
    b.extend(1u16.to_le_bytes());
    b.extend(2u16.to_le_bytes());
    b.extend(rate.to_le_bytes());
    b.extend((rate * 4).to_le_bytes());
    b.extend(4u16.to_le_bytes());
    b.extend(16u16.to_le_bytes());
    b.extend(b"data");
    b.extend((frames * 4).to_le_bytes());
    for _ in 0..frames * 2 {
        b.extend(sample.to_le_bytes());
    }
    Arc::new(Bytes(b))
}
fn item(key: &str, frames: u32, sample: i16, crossfade: f64) -> Item {
    Item { key: key.into(), source: wav(frames, 48000, sample), ext: "wav".into(), gain: 1.0, crossfade }
}
#[test]
fn gapless_marks_and_samples_cross_read_boundaries() {
    let mut s = Sequencer::new(48000);
    s.play(vec![item("a", 5000, 8192, 0.0), item("b", 6000, 16384, 0.0)], 0.0, 0);
    let mut all = Vec::new();
    let mut out = vec![0.0; 4096 * 2];
    loop {
        let n = s.read(&mut out);
        all.extend_from_slice(&out[..n * 2]);
        if n < 4096 {
            break;
        }
    }
    assert_eq!(all.len(), 11000 * 2);
    assert!(all[..10000].iter().all(|x| (*x - 0.25).abs() < 0.001));
    assert!(all[10000..].iter().all(|x| (*x - 0.5).abs() < 0.001));
    let events = s.events();
    assert!(events.iter().any(|e| matches!(e,Event::Mark {key,frame:5000,..} if key=="b")));
    assert!(matches!(events.last(), Some(Event::End { frame: 11000 })));
}
#[test]
fn crossfade_has_exact_overlap_and_equal_power() {
    let mut s = Sequencer::new(48000);
    s.play(vec![item("a", 48000, 8192, 0.25), item("b", 48000, 8192, 0.0)], 0.0, 0);
    let mut out = vec![0.0; 96000 * 2];
    let n = s.read(&mut out);
    assert_eq!(n, 84000);
    assert!((out[42000 * 2] - 0.25 * 2f32.sqrt()).abs() < 0.001);
    assert!(s.events().iter().any(|e| matches!(e,Event::Mark {key,frame:36000,..} if key=="b")));
}
#[test]
fn failed_sources_are_skipped_with_an_error() {
    let mut bad = item("bad", 100, 0, 0.0);
    bad.source = Arc::new(Bytes(vec![0; 128]));
    let mut s = Sequencer::new(48000);
    s.play(vec![bad, item("good", 100, 8192, 0.0)], 0.0, 0);
    assert_eq!(s.read(&mut [0.0; 400]), 100);
    assert!(s.events().iter().any(|e| matches!(e,Event::Error {key,..} if key=="bad")));
}
#[test]
fn resampled_length_and_gain() {
    struct Shared(Arc<dyn RangeSource>);
    impl RangeSource for Shared {
        fn len(&self) -> io::Result<u64> {
            self.0.len()
        }

        fn read_range(&self, a: u64, b: u64) -> io::Result<Vec<u8>> {
            self.0.read_range(a, b)
        }
    }
    let mut t = Track::new(Box::new(Shared(wav(44100, 44100, 8192))), "wav", 48000).unwrap();
    t.set_gain(0.5);
    let mut out = vec![0.0; 50000 * 2];
    assert_eq!(t.read(&mut out).unwrap(), 48000);
    assert!((out[24000 * 2] - 0.125).abs() < 0.002);
}

#[test]
fn queue_changes_replace_unheard_decoded_tracks() {
    let mut s = Sequencer::new(48000);
    s.play(vec![item("a", 5000, 8192, 0.0), item("b", 5000, 16384, 0.0)], 0.0, 0);
    assert_eq!(s.read(&mut [0.0; 8192 * 2]), 8192);
    s.events();
    assert_eq!(s.upcoming("a", vec![item("c", 5000, 24576, 0.0)], 2000), Some(5000));
    let mut out = [0.0; 6000 * 2];
    assert_eq!(s.read(&mut out), 5000);
    assert!(out[..10000].iter().all(|x| (*x - 0.75).abs() < 0.001));
    assert!(s.events().iter().any(|e| matches!(e,Event::Mark{key,frame:5000,..}if key=="c")));
}
#[test]
fn appending_after_decoder_finished_keeps_the_boundary() {
    let mut s = Sequencer::new(48000);
    s.play(vec![item("a", 100, 8192, 0.0)], 0.0, 0);
    assert_eq!(s.read(&mut [0.0; 400]), 100);
    s.upcoming("a", vec![item("b", 100, 16384, 0.0)], 50);
    assert_eq!(s.read(&mut [0.0; 400]), 100);
    assert!(s.events().iter().any(|e| matches!(e,Event::Mark{key,frame:100,..}if key=="b")));
}

#[test]
fn mp3_encoder_delay_and_padding_are_trimmed() {
    let mut track = Track::new(Box::new(Bytes(include_bytes!("fixtures/tone.mp3").to_vec())), "mp3", 48000).unwrap();
    let mut out = vec![0.0; 50000 * 2];
    assert_eq!(track.read(&mut out).unwrap(), 48000);
}
#[cfg(feature = "opus")]
#[test]
fn opus_preskip_and_end_padding_are_trimmed() {
    let mut track = Track::new(Box::new(Bytes(include_bytes!("fixtures/tone.opus").to_vec())), "opus", 48000).unwrap();
    let mut out = vec![0.0; 50000 * 2];
    assert_eq!(track.read(&mut out).unwrap(), 48000);
    assert!(out[..48000 * 2].iter().any(|x| x.abs() > 0.01));
}

#[test]
fn replenishing_an_unchanged_queue_preserves_the_end_mark() {
    let mut s = Sequencer::new(48000);
    s.play(vec![item("a", 100, 8192, 0.0), item("b", 100, 8192, 0.0)], 0.0, 0);
    assert_eq!(s.read(&mut [0.0; 600]), 200);
    s.events();
    s.upcoming("a", vec![item("b", 100, 8192, 0.0)], 0);
    assert!(s.events().iter().any(|e| matches!(e, Event::End { frame: 200 })));
}

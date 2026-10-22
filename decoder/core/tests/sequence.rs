// SPDX-License-Identifier: AGPL-3.0-or-later

use std::io;
use std::path::PathBuf;
use std::sync::Arc;

use tinystream_decoder_core::sequence::{Event, Item, Sequencer};
use tinystream_decoder_core::{RangeSource, Track};

struct File(Vec<u8>);

impl RangeSource for File {
    fn len(&self) -> io::Result<u64> {
        Ok(self.0.len() as u64)
    }

    fn read_range(&self, start: u64, end: u64) -> io::Result<Vec<u8>> {
        Ok(self.0[start as usize..=end as usize].to_vec())
    }
}

fn source(name: &str) -> Arc<dyn RangeSource> {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures").join(name);
    Arc::new(File(std::fs::read(path).unwrap()))
}

fn item(key: &str, name: &str) -> Item {
    let ext = name.rsplit('.').next().unwrap().to_string();
    Item { key: key.into(), source: source(name), size: 0, ext, gain: 1.0, crossfade: 0.0 }
}

fn decode(track: &mut Track) -> usize {
    let mut out = vec![0.0; 4096 * 2];
    let mut total = 0;

    loop {
        let n = track.read(&mut out).unwrap();
        total += n;

        if n < 4096 {
            return total;
        }
    }
}

/// Everything there is, in reads of `chunk` frames.
fn drain(seq: &mut Sequencer, chunk: usize) -> Vec<f32> {
    let mut all = Vec::new();
    let mut out = vec![0.0; chunk * 2];

    loop {
        let n = seq.read(&mut out);
        all.extend_from_slice(&out[..n * 2]);

        if n < chunk {
            return all;
        }
    }
}

fn marks(events: &[Event]) -> Vec<(String, u64)> {
    events
        .iter()
        .filter_map(|e| match e {
            Event::Mark { key, frame, .. } => Some((key.clone(), *frame)),
            _ => None,
        })
        .collect()
}

#[test]
fn decodes_every_frame() {
    let mut t = Track::open(source("a.flac"), 0, "flac", 44100).unwrap();
    assert_eq!(decode(&mut t), 22050);
    assert!((t.duration() - 0.5).abs() < 1e-9);
}

#[test]
fn resamples_to_the_exact_length() {
    let mut t = Track::open(source("a.flac"), 0, "flac", 48000).unwrap();
    assert_eq!(decode(&mut t), 24000);
}

#[test]
fn trims_mp3_delay_and_padding() {
    let mut t = Track::open(source("a.mp3"), 0, "mp3", 44100).unwrap();
    assert_eq!(decode(&mut t), 22050);
}

#[cfg(feature = "opus")]
#[test]
fn plays_opus_without_its_pre_skip() {
    let mut t = Track::open(source("a.opus"), 0, "opus", 48000).unwrap();
    assert_eq!(decode(&mut t), 24000);
}

#[test]
fn seeks_to_the_sample() {
    let mut t = Track::open(source("a.flac"), 0, "flac", 44100).unwrap();
    t.seek(0.25).unwrap();
    assert_eq!(decode(&mut t), 22050 - 11025);
}

#[test]
fn plays_one_straight_into_the_next() {
    let mut seq = Sequencer::new(44100);
    seq.play(vec![item("a", "a.flac"), item("b", "b.flac")], 0.0, 1000);
    let all = drain(&mut seq, 3000);
    let events = seq.events();
    assert_eq!(all.len() / 2, 22050 + 33075);
    assert_eq!(marks(&events), [("a".into(), 1000), ("b".into(), 1000 + 22050)]);
    assert_eq!(events.last(), Some(&Event::End { frame: 1000 + 22050 + 33075 }));
    assert!(seq.finished());
}

#[test]
fn starts_part_way_in() {
    let mut seq = Sequencer::new(44100);
    seq.play(vec![item("a", "a.flac")], 0.25, 0);
    assert_eq!(drain(&mut seq, 4096).len() / 2, 11025);
    assert!(matches!(seq.events()[0], Event::Mark { offset: 0.25, .. }));
}

#[test]
fn skips_what_it_cant_open() {
    let mut seq = Sequencer::new(44100);
    seq.play(vec![item("x", "broken.flac"), item("a", "a.flac")], 0.0, 0);
    assert_eq!(drain(&mut seq, 4096).len() / 2, 22050);
    let events = seq.events();
    assert!(matches!(&events[0], Event::Error { key, .. } if key == "x"));
    assert_eq!(marks(&events), [("a".into(), 0)]);
}

#[test]
fn crossfades_into_the_next() {
    let mut seq = Sequencer::new(44100);
    let mut a = item("a", "a.flac");
    a.crossfade = 0.1;
    seq.play(vec![a, item("b", "b.flac")], 0.0, 0);
    let all = drain(&mut seq, 1024);
    let marks = marks(&seq.events());
    let overlap = 22050 + 33075 - all.len() / 2;
    assert_eq!(overlap, 4410);
    assert_eq!(marks[1], ("b".into(), (22050 - overlap) as u64));
}

#[test]
fn applies_gain() {
    let mut seq = Sequencer::new(44100);
    seq.play(vec![item("a", "a.flac")], 0.0, 0);
    let loud = drain(&mut seq, 4096);
    seq.play(vec![item("a", "a.flac")], 0.0, 0);
    seq.gain("a", 0.5);
    let quiet = drain(&mut seq, 4096);
    assert!(loud.iter().zip(&quiet).all(|(l, q)| (l * 0.5 - q).abs() < 1e-6));
}

#[test]
fn redoes_what_has_not_been_heard() {
    let mut seq = Sequencer::new(44100);
    seq.play(vec![item("a", "a.flac"), item("b", "b.flac")], 0.0, 0);
    let mut out = vec![0.0; 30000 * 2];
    assert_eq!(seq.read(&mut out), 30000);
    seq.events();

    // b began at 22050 but only 20000 have been heard: it goes, c comes instead.
    assert_eq!(seq.upcoming("a", vec![item("c", "c.flac")], 20000), Some(22050));
    let rest = drain(&mut seq, 4096);
    let events = seq.events();
    assert_eq!(events[0], Event::Rewind { frame: 22050 });
    assert_eq!(marks(&events), [("c".into(), 22050)]);
    assert_eq!(rest.len() / 2, 22050 + 22050 - 22050);
}

#[test]
fn keeps_what_has_been_heard() {
    let mut seq = Sequencer::new(44100);
    seq.play(vec![item("a", "a.flac"), item("b", "b.flac")], 0.0, 0);
    let mut out = vec![0.0; 30000 * 2];
    seq.read(&mut out);
    assert_eq!(seq.upcoming("a", vec![item("c", "c.flac")], 25000), None);
}

#[test]
fn lines_up_what_comes_later_without_redoing_anything() {
    let mut seq = Sequencer::new(44100);
    seq.play(vec![item("a", "a.flac")], 0.0, 0);
    let mut out = vec![0.0; 10000 * 2];
    seq.read(&mut out);
    assert_eq!(seq.upcoming("a", vec![item("b", "b.flac")], 0), None);
    assert_eq!(drain(&mut seq, 4096).len() / 2, 22050 + 33075 - 10000);
}

#[test]
fn carries_on_with_what_is_added_after_the_end() {
    let mut seq = Sequencer::new(44100);
    seq.play(vec![item("a", "a.flac")], 0.0, 0);
    drain(&mut seq, 4096);
    assert!(seq.finished());
    seq.events();
    assert_eq!(seq.upcoming("a", vec![item("b", "b.flac")], 22050), None);
    assert!(!seq.finished());
    assert_eq!(drain(&mut seq, 4096).len() / 2, 33075);
    assert_eq!(marks(&seq.events()), [("b".into(), 22050)]);
}

#[test]
fn undoes_a_fade_into_what_changed() {
    let mut seq = Sequencer::new(44100);
    let mut a = item("a", "a.flac");
    a.crossfade = 0.2;
    seq.play(vec![a, item("b", "b.flac")], 0.0, 0);
    let mut out = vec![0.0; 1024 * 2];

    while seq.written() < 20000 {
        seq.read(&mut out);
    }

    assert_eq!(marks(&seq.events()).len(), 2, "the fade has begun");

    let mut fresh = Sequencer::new(44100);
    fresh.play(vec![item("a", "a.flac")], 0.0, 0);
    let alone = drain(&mut fresh, 4096);

    let back = seq.upcoming("a", vec![], 0).expect("rewinds");
    let rest = drain(&mut seq, 4096);
    // The rest of a, untouched by the fade.
    assert_eq!(rest.len() / 2, 22050 - back as usize);
    assert!(alone[back as usize * 2..].iter().zip(&rest).all(|(x, y)| (x - y).abs() < 1e-3));
}

// SPDX-License-Identifier: AGPL-3.0-or-later

use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};

/// Seconds of sound decoded ahead, as on web.
pub const SECONDS: usize = 8;

/// Stereo frames between the decoder (the producer) and the speakers (the consumer). Counters only
/// grow, except when the producer takes back frames that haven't been played; a frame's place is
/// its count modulo the ring's size. Silence during starvation does not advance media time.
pub struct Ring {
    samples: Vec<AtomicU32>,
    read: AtomicU64,
    written: AtomicU64,
    /// Bumped by the producer to throw away what's buffered, e.g. after a seek.
    generation: AtomicU64,
    /// Where playing picks up after the generation changes.
    flush_at: AtomicU64,
}
impl Ring {
    pub fn new(frames: usize) -> Self {
        Self {
            samples: (0..frames.max(1) * 2).map(|_| AtomicU32::new(0)).collect(),
            read: AtomicU64::new(0),
            written: AtomicU64::new(0),
            generation: AtomicU64::new(0),
            flush_at: AtomicU64::new(0),
        }
    }

    pub fn heard(&self) -> u64 {
        self.read.load(Ordering::Acquire)
    }

    pub fn written(&self) -> u64 {
        self.written.load(Ordering::Acquire)
    }

    pub fn capacity(&self) -> u64 {
        self.samples.len() as u64 / 2
    }

    /// Frames that fit before reaching what hasn't been played yet.
    pub fn room(&self) -> u64 {
        self.capacity().saturating_sub(self.written().saturating_sub(self.heard()))
    }

    /// Throws away everything not played yet; the frame writing carries on from.
    pub fn flush(&self) -> u64 {
        let at = self.written();
        self.flush_at.store(at, Ordering::Release);
        self.generation.fetch_add(1, Ordering::AcqRel);
        at
    }

    /// Takes back what was written from `frame` on, which mustn't have been played.
    pub fn rewind(&self, frame: u64) {
        if frame < self.written() {
            self.written.store(frame.max(self.heard()), Ordering::Release);
        }
    }

    pub fn write(&self, samples: &[f32]) -> usize {
        let written = self.written.load(Ordering::Relaxed);
        let frames = (samples.len() / 2).min(self.room() as usize);
        for (i, sample) in samples[..frames * 2].iter().enumerate() {
            self.samples[((written * 2 + i as u64) % self.samples.len() as u64) as usize]
                .store(sample.to_bits(), Ordering::Relaxed);
        }
        self.written.store(written + frames as u64, Ordering::Release);
        frames
    }

    /// The next frame to play; `seen` is the generation the consumer last played from.
    pub fn frame(&self, seen: &mut u64) -> Option<[f32; 2]> {
        let generation = self.generation.load(Ordering::Acquire);
        if generation != *seen {
            *seen = generation;
            self.read.store(self.flush_at.load(Ordering::Acquire), Ordering::Release);
        }
        let read = self.read.load(Ordering::Relaxed);
        if read >= self.written() {
            return None;
        }
        let i = (read % self.capacity()) as usize * 2;
        let frame = [
            f32::from_bits(self.samples[i].load(Ordering::Relaxed)),
            f32::from_bits(self.samples[i + 1].load(Ordering::Relaxed)),
        ];
        self.read.store(read + 1, Ordering::Release);
        Some(frame)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn wrapping_and_starvation_preserve_media_time() {
        let ring = Ring::new(2);
        let mut seen = 0;
        assert_eq!(ring.frame(&mut seen), None);
        assert_eq!(ring.heard(), 0);
        assert_eq!(ring.write(&[1.0, 2.0, 3.0, 4.0, 5.0, 6.0]), 2);
        assert_eq!(ring.frame(&mut seen), Some([1.0, 2.0]));
        assert_eq!(ring.write(&[5.0, 6.0]), 1);
        assert_eq!(ring.frame(&mut seen), Some([3.0, 4.0]));
        assert_eq!(ring.frame(&mut seen), Some([5.0, 6.0]));
        assert_eq!(ring.frame(&mut seen), None);
        assert_eq!(ring.heard(), 3);
    }
    #[test]
    fn flushing_skips_what_was_buffered_and_rewinding_replaces_it() {
        let ring = Ring::new(8);
        let mut seen = 0;
        ring.write(&[1.0, 1.0, 2.0, 2.0, 3.0, 3.0]);
        assert_eq!(ring.frame(&mut seen), Some([1.0, 1.0]));
        assert_eq!(ring.flush(), 3);
        ring.write(&[7.0, 7.0, 8.0, 8.0]);
        assert_eq!(ring.frame(&mut seen), Some([7.0, 7.0]));
        assert_eq!(ring.heard(), 4);
        ring.rewind(4);
        ring.write(&[9.0, 9.0]);
        assert_eq!(ring.frame(&mut seen), Some([9.0, 9.0]));
        assert_eq!(ring.frame(&mut seen), None);
        ring.rewind(2);
        assert_eq!(ring.written(), 5, "Never takes back what's been played");
    }
}

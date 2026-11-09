// SPDX-License-Identifier: AGPL-3.0-or-later

use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};

/// One producer and one consumer; silence during starvation does not advance media time.
pub struct Ring {
    samples: Vec<AtomicU32>,
    read: AtomicU64,
    written: AtomicU64,
}
impl Ring {
    pub fn new(frames: usize) -> Self {
        Self {
            samples: (0..frames.max(1) * 2).map(|_| AtomicU32::new(0)).collect(),
            read: AtomicU64::new(0),
            written: AtomicU64::new(0),
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

    pub fn write(&self, samples: &[f32]) -> usize {
        let written = self.written.load(Ordering::Relaxed);
        let free = self.capacity().saturating_sub(written.saturating_sub(self.heard()));
        let frames = (samples.len() / 2).min(free as usize);
        for i in 0..frames * 2 {
            self.samples[((written * 2 + i as u64) % self.samples.len() as u64) as usize]
                .store(samples[i].to_bits(), Ordering::Relaxed);
        }
        self.written.store(written + frames as u64, Ordering::Release);
        frames
    }

    pub fn frame(&self) -> Option<[f32; 2]> {
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
        assert_eq!(ring.frame(), None);
        assert_eq!(ring.heard(), 0);
        assert_eq!(ring.write(&[1.0, 2.0, 3.0, 4.0, 5.0, 6.0]), 2);
        assert_eq!(ring.frame(), Some([1.0, 2.0]));
        assert_eq!(ring.write(&[5.0, 6.0]), 1);
        assert_eq!(ring.frame(), Some([3.0, 4.0]));
        assert_eq!(ring.frame(), Some([5.0, 6.0]));
        assert_eq!(ring.frame(), None);
        assert_eq!(ring.heard(), 3);
    }
}

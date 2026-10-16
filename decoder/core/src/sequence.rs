// SPDX-License-Identifier: AGPL-3.0-or-later
//! A queue decoded one track straight into the next, so there's never a gap between them, or
//! crossfading into the next when asked to. Frames are counted from where playing began, so the
//! player can tell where each track starts in what it's heard.

use std::f32::consts::FRAC_PI_2;
use std::sync::Arc;

use crate::{Error, RangeSource, Track};

/// Frames decoded at a time.
const CHUNK: usize = 4096;

#[derive(Clone)]
pub struct Item {
    pub key: String,
    pub source: Arc<dyn RangeSource>,
    /// Bytes, or 0 to ask the source.
    pub size: u64,
    pub ext: String,
    /// Linear; ReplayGain with clipping kept off.
    pub gain: f32,
    /// Seconds to overlap with the next item; 0 for none.
    pub crossfade: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Event {
    /// `key` begins at `frame`, `offset` seconds in.
    Mark { key: String, frame: u64, offset: f64, duration: f64 },
    /// Nothing more after `frame`.
    End { frame: u64 },
    /// Everything from `frame` on is thrown away; what comes next replaces it.
    Rewind { frame: u64 },
    /// `key` couldn't be played, or not to the end.
    Error { key: String, message: String },
}

struct Playing {
    track: Track,
    key: String,
    /// The frame it began at, `offset` seconds in.
    began: u64,
    offset: f64,
}

struct Fade {
    next: Playing,
    total: u64,
    done: u64,
}

pub struct Sequencer {
    rate: u32,
    list: Vec<Item>,
    index: usize,
    current: Option<Playing>,
    fade: Option<Fade>,
    /// Where each started item began, to undo what's not been heard yet.
    started: Vec<(String, u64)>,
    written: u64,
    finished: bool,
    events: Vec<Event>,
    mix: Vec<f32>,
}

impl Sequencer {
    pub fn new(rate: u32) -> Self {
        Self {
            rate,
            list: Vec::new(),
            index: 0,
            current: None,
            fade: None,
            started: Vec::new(),
            written: 0,
            finished: true,
            events: Vec::new(),
            mix: vec![0.0; CHUNK * 2],
        }
    }

    /// The rate it decodes to.
    pub fn rate(&self) -> u32 {
        self.rate
    }

    /// Frames decoded so far, counting from where [`Sequencer::play`] said to.
    pub fn written(&self) -> u64 {
        self.written
    }

    /// Whether everything's been decoded.
    pub fn finished(&self) -> bool {
        self.finished
    }

    /// What happened since it was last asked.
    pub fn events(&mut self) -> Vec<Event> {
        std::mem::take(&mut self.events)
    }

    /// Throws away whatever was lined up and decodes `items` in order, from `start` seconds into
    /// the first, counting frames on from `frame`.
    pub fn play(&mut self, items: Vec<Item>, start: f64, frame: u64) {
        self.current = None;
        self.fade = None;
        self.list = items;
        self.index = 0;
        self.started.clear();
        self.events.clear();
        self.finished = false;
        self.written = frame;
        self.begin(start);
    }

    /// Changes what follows `after`. What's been decoded but not `heard` yet is redone: then it
    /// gives the frame it went back to, and everything from there on has to go.
    pub fn upcoming(&mut self, after: &str, items: Vec<Item>, heard: u64) -> Option<u64> {
        let at = self.list.iter().position(|i| i.key == after)?;
        let old = std::mem::take(&mut self.list);
        self.list = old[..=at].iter().cloned().chain(items).collect();

        let len = self.list.len().max(old.len());
        let changed = (0..len).find(|&i| old.get(i).map(|o| &o.key) != self.list.get(i).map(|l| &l.key))?;
        let begun = if self.current.is_some() { self.index + 1 + self.fade.is_some() as usize } else { self.index };

        if changed >= begun {
            if self.current.is_none() && self.index < self.list.len() {
                self.finished = false;
                self.begin(0.0);
            }

            return None;
        }

        let key = &old[changed].key;
        let &(_, frame) = self.started.iter().rev().find(|(k, _)| k == key)?;

        if frame <= heard {
            return None;
        }

        self.written = frame;
        self.started.retain(|&(_, f)| f < frame);
        self.events.push(Event::Rewind { frame });
        self.fade = None;

        if changed <= self.index {
            self.current = None;
            self.index = changed;
            self.finished = false;
            self.begin(0.0);
        } else if let Some(c) = &mut self.current {
            // Only the next one changed, while fading into it: back to before the fade began.
            let at = c.offset + (frame - c.began) as f64 / self.rate as f64;

            if let Err(e) = c.track.seek(at) {
                self.events.push(Event::Error { key: c.key.clone(), message: e.0 });
            }

            c.began = frame;
            c.offset = at;
        }

        Some(frame)
    }

    pub fn gain(&mut self, key: &str, gain: f32) {
        self.list.iter_mut().filter(|i| i.key == key).for_each(|i| i.gain = gain);
        let fading = self.fade.as_mut().map(|f| &mut f.next);

        for p in self.current.iter_mut().chain(fading) {
            if p.key == key {
                p.track.gain = gain;
            }
        }
    }

    pub fn stop(&mut self) {
        self.current = None;
        self.fade = None;
        self.list.clear();
        self.finished = true;
    }

    /// Fills `out` with stereo frames; returns how many, fewer only when there's nothing left.
    pub fn read(&mut self, out: &mut [f32]) -> usize {
        let want = out.len() / 2;
        let mut done = 0;

        while done < want && self.current.is_some() {
            let n = (want - done).min(CHUNK);
            done += self.step(&mut out[done * 2..(done + n) * 2]);
        }

        done
    }

    fn open(&self, item: &Item, start: f64) -> Result<Track, Error> {
        let mut track = Track::open(item.source.clone(), item.size, &item.ext, self.rate)?;
        track.gain = item.gain;

        if start > 0.0 {
            track.seek(start)?;
        }

        Ok(track)
    }

    /// Starts the item at `index`, or the next one that opens.
    fn begin(&mut self, mut start: f64) {
        while let Some(item) = self.list.get(self.index) {
            match self.open(item, start) {
                Ok(track) => {
                    let key = item.key.clone();
                    let frame = self.written;
                    self.started.push((key.clone(), frame));
                    self.events.push(Event::Mark {
                        key: key.clone(),
                        frame,
                        offset: start,
                        duration: track.duration(),
                    });
                    self.current = Some(Playing { track, key, began: frame, offset: start });
                    return;
                },
                Err(e) => {
                    self.events.push(Event::Error { key: item.key.clone(), message: e.0 });
                    self.index += 1;
                    start = 0.0;
                },
            }
        }

        self.current = None;
        self.finished = true;
        self.events.push(Event::End { frame: self.written });
    }

    fn pull(p: &mut Playing, out: &mut [f32], events: &mut Vec<Event>) -> usize {
        p.track.read(out).unwrap_or_else(|e| {
            events.push(Event::Error { key: p.key.clone(), message: e.0 });
            0
        })
    }

    fn step(&mut self, out: &mut [f32]) -> usize {
        let frames = out.len() / 2;
        let Some(current) = &mut self.current else { return 0 };

        if let Some(fade) = &mut self.fade {
            let n = Self::pull(current, out, &mut self.events);
            let mix = &mut self.mix[..frames * 2];
            let m = Self::pull(&mut fade.next, mix, &mut self.events);
            let len = n.max(m);
            out[n * 2..len * 2].fill(0.0);
            mix[m * 2..len * 2].fill(0.0);

            for i in 0..len {
                let x = ((fade.done + i as u64) as f32 / fade.total as f32).min(1.0);
                let (from, to) = ((x * FRAC_PI_2).cos(), (x * FRAC_PI_2).sin());
                out[i * 2] = out[i * 2] * from + mix[i * 2] * to;
                out[i * 2 + 1] = out[i * 2 + 1] * from + mix[i * 2 + 1] * to;
            }

            fade.done += len as u64;
            self.written += len as u64;

            if n < frames {
                self.current = self.fade.take().map(|f| f.next);
                self.index += 1;
            }

            return len;
        }

        // Up to where the fade begins, so it begins right there.
        let total = self.fade_length();
        let upto = match self.until_fade(total) {
            Some(left) => left.clamp(1, frames),
            None => frames,
        };
        let Some(current) = &mut self.current else { return 0 };
        let n = Self::pull(current, &mut out[..upto * 2], &mut self.events);
        self.written += n as u64;

        if n < upto {
            self.current = None;
            self.index += 1;
            self.begin(0.0);
            return n;
        }

        let Some(next) = self.list.get(self.index + 1) else { return n };

        // Close enough to the end to start mixing in the next one.
        if total > 0 && self.until_fade(total) == Some(0) {
            match self.open(next, 0.0) {
                Ok(track) => {
                    let (key, frame) = (next.key.clone(), self.written);
                    self.started.push((key.clone(), frame));
                    self.events.push(Event::Mark { key: key.clone(), frame, offset: 0.0, duration: track.duration() });
                    self.fade = Some(Fade { next: Playing { track, key, began: frame, offset: 0.0 }, total, done: 0 });
                },
                Err(e) => {
                    self.events.push(Event::Error { key: next.key.clone(), message: e.0 });
                    self.list.remove(self.index + 1);
                },
            }
        }

        n
    }

    /// Frames long the fade out of the current item is, if it fades into another.
    fn fade_length(&self) -> u64 {
        match self.list.get(self.index) {
            Some(item) if self.list.len() > self.index + 1 => (item.crossfade * self.rate as f64).round() as u64,
            _ => 0,
        }
    }

    /// Frames to go before the current item's last `total` begin; its length is the decoder's best
    /// guess, and without one there's no fading.
    fn until_fade(&self, total: u64) -> Option<usize> {
        let c = self.current.as_ref().filter(|c| total > 0 && c.track.duration() > 0.0)?;
        let rate = self.rate as f64;
        let done = (self.written - c.began) as f64 + c.offset * rate;
        Some((c.track.duration() * rate - total as f64 - done).ceil().max(0.0) as usize)
    }
}

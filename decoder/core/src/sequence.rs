// SPDX-License-Identifier: AGPL-3.0-or-later

use std::collections::VecDeque;
use std::sync::Arc;

use crate::{DecoderError, RangeSource, Track};

#[derive(Clone)]
pub struct Item {
    pub key: String,
    pub source: Arc<dyn RangeSource>,
    pub ext: String,
    pub gain: f32,
    pub crossfade: f64,
}

#[derive(Clone, Debug)]
pub enum Event {
    Mark { key: String, frame: u64, offset: f64, duration: f64 },
    End { frame: u64 },
    Rewind { frame: u64 },
    Error { key: String, message: String },
}

struct Source(Arc<dyn RangeSource>);
impl RangeSource for Source {
    fn len(&self) -> std::io::Result<u64> {
        self.0.len()
    }

    fn read_range(&self, start: u64, end: u64) -> std::io::Result<Vec<u8>> {
        self.0.read_range(start, end)
    }
}

struct Active {
    track: Track,
    item: Item,
    offset: f64,
    consumed: u64,
}
struct Fade {
    next: Active,
    total: u64,
    done: u64,
}

#[derive(Clone)]
struct Started {
    item: Item,
    frame: u64,
    offset: f64,
}

pub struct Sequencer {
    rate: u32,
    frame: u64,
    queue: VecDeque<Item>,
    started: Vec<Started>,
    current: Option<Active>,
    fade: Option<Fade>,
    events: Vec<Event>,
    ended: bool,
}

impl Sequencer {
    pub fn new(rate: u32) -> Self {
        Self {
            rate,
            frame: 0,
            queue: VecDeque::new(),
            started: Vec::new(),
            current: None,
            fade: None,
            events: Vec::new(),
            ended: false,
        }
    }

    pub fn play(&mut self, items: Vec<Item>, start: f64, frame: u64) {
        self.stop();
        self.frame = frame;
        self.queue = items.into();
        self.ended = false;
        self.begin(start);
    }

    fn open(&mut self, item: Item, start: f64) -> Result<Active, DecoderError> {
        let mut track = Track::new(Box::new(Source(item.source.clone())), &item.ext, self.rate)?;
        track.set_gain(item.gain);
        if start > 0.0 {
            track.seek(start)?;
        }
        self.started.push(Started { item: item.clone(), frame: self.frame, offset: start });
        self.events.push(Event::Mark {
            key: item.key.clone(),
            frame: self.frame,
            offset: start,
            duration: track.duration(),
        });
        Ok(Active { track, item, offset: start, consumed: 0 })
    }

    fn begin(&mut self, mut start: f64) {
        while let Some(item) = self.queue.pop_front() {
            match self.open(item.clone(), start) {
                Ok(active) => {
                    self.current = Some(active);
                    return;
                },
                Err(e) => self.events.push(Event::Error { key: item.key, message: e.to_string() }),
            }
            start = 0.0;
        }
        if !self.ended {
            self.events.push(Event::End { frame: self.frame });
            self.ended = true;
        }
    }

    pub fn upcoming(&mut self, after: &str, items: Vec<Item>, heard: u64) -> Option<u64> {
        let at = self
            .started
            .iter()
            .rposition(|s| s.item.key == after && s.frame <= heard)
            .or_else(|| self.started.iter().rposition(|s| s.item.key == after))?;
        let decoded = &self.started[at + 1..];
        let old: Vec<_> = decoded.iter().map(|s| &s.item).chain(self.queue.iter()).collect();
        let common = old.iter().zip(&items).take_while(|(a, b)| a.key == b.key && a.ext == b.ext).count();
        if common < decoded.len() {
            let rewind = decoded[common].frame;
            if rewind <= heard {
                return None;
            }
            let previous = self.started[at + common].clone();
            let offset = previous.offset + (rewind - previous.frame) as f64 / self.rate as f64;
            self.current = None;
            self.fade = None;
            self.frame = rewind;
            self.started.truncate(at + common + 1);
            self.queue = items.into_iter().skip(common).collect();
            self.events
                .retain(|e| matches!(e, Event::Mark {frame,..} if *frame < rewind) || matches!(e, Event::Error { .. }));
            self.events.push(Event::Rewind { frame: rewind });
            match Track::new(Box::new(Source(previous.item.source.clone())), &previous.item.ext, self.rate).and_then(
                |mut track| {
                    track.seek(offset)?;
                    track.set_gain(previous.item.gain);
                    Ok(track)
                },
            ) {
                Ok(track) => {
                    self.current = Some(Active {
                        track,
                        item: previous.item,
                        offset: previous.offset,
                        consumed: rewind - previous.frame,
                    })
                },
                Err(e) => self.events.push(Event::Error { key: previous.item.key, message: e.to_string() }),
            }
            self.ended = false;
            return Some(rewind);
        }
        self.queue = items.into_iter().skip(decoded.len()).collect();
        if !self.queue.is_empty() {
            self.ended = false;
        } else if self.ended {
            self.events.push(Event::End { frame: self.frame });
        }
        if at > 8 {
            self.started.drain(..at - 4);
        }
        None
    }

    pub fn gain(&mut self, key: &str, gain: f32) {
        for s in &mut self.started {
            if s.item.key == key {
                s.item.gain = gain;
            }
        }
        for item in &mut self.queue {
            if item.key == key {
                item.gain = gain;
            }
        }
        if let Some(current) = &mut self.current {
            if current.item.key == key {
                current.track.set_gain(gain);
            }
        }
        if let Some(fade) = &mut self.fade {
            if fade.next.item.key == key {
                fade.next.track.set_gain(gain);
            }
        }
    }

    pub fn stop(&mut self) {
        self.queue.clear();
        self.started.clear();
        self.current = None;
        self.fade = None;
        self.events.clear();
        self.ended = true;
    }

    pub fn events(&mut self) -> Vec<Event> {
        std::mem::take(&mut self.events)
    }

    pub fn frame(&self) -> u64 {
        self.frame
    }

    fn read_track(events: &mut Vec<Event>, active: &mut Active, out: &mut [f32]) -> usize {
        match active.track.read(out) {
            Ok(n) => {
                active.consumed += n as u64;
                n as usize
            },
            Err(e) => {
                events.push(Event::Error { key: active.item.key.clone(), message: e.to_string() });
                0
            },
        }
    }

    pub fn read(&mut self, out: &mut [f32]) -> usize {
        let want = out.len() / 2;
        let mut written = 0;
        while written < want {
            if self.current.is_none() {
                self.begin(0.0);
                if self.current.is_none() {
                    break;
                }
            }
            if self.fade.is_none() {
                let current = self.current.as_ref().unwrap();
                let remaining = ((current.track.duration() - current.offset) * self.rate as f64).round() as u64;
                let total = (current.item.crossfade.max(0.0) * self.rate as f64).round() as u64;
                if total > 0 && remaining > 0 && current.consumed.saturating_add(total) >= remaining {
                    while let Some(next) = self.queue.pop_front() {
                        match self.open(next.clone(), 0.0) {
                            Ok(next) => {
                                self.fade = Some(Fade { next, total: total.min(remaining), done: 0 });
                                break;
                            },
                            Err(e) => self.events.push(Event::Error { key: next.key, message: e.to_string() }),
                        }
                    }
                }
            }
            let current = self.current.as_mut().unwrap();
            let mut count = want - written;
            if self.fade.is_none()
                && !self.queue.is_empty()
                && current.item.crossfade > 0.0
                && current.track.duration() > 0.0
            {
                let until = ((current.track.duration() - current.offset - current.item.crossfade).max(0.0)
                    * self.rate as f64)
                    .round() as u64;
                if until > current.consumed {
                    count = count.min((until - current.consumed) as usize);
                }
            }
            let dst = &mut out[written * 2..(written + count) * 2];
            dst.fill(0.0);
            let n = Self::read_track(&mut self.events, current, dst);
            let frames;
            if let Some(fade) = &mut self.fade {
                let mut other = vec![0.0; count * 2];
                let m = Self::read_track(&mut self.events, &mut fade.next, &mut other);
                frames = n.max(m);
                for i in 0..frames {
                    let x = ((fade.done + i as u64) as f64 / fade.total as f64).min(1.0);
                    let (into, away) = (x * std::f64::consts::FRAC_PI_2).sin_cos();
                    for c in 0..2 {
                        dst[i * 2 + c] = dst[i * 2 + c] * away as f32 + other[i * 2 + c] * into as f32;
                    }
                }
                fade.done += frames as u64;
                if n < count || fade.done >= fade.total {
                    self.current = Some(self.fade.take().unwrap().next);
                }
            } else {
                frames = n;
                if n < count {
                    self.current = None;
                }
            }
            written += frames;
            self.frame += frames as u64;
        }
        written
    }
}

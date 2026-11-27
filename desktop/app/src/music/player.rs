// SPDX-License-Identifier: AGPL-3.0-or-later
//! The music player the apps share (`packages/shared/src/music.ts`): what's queued, what's playing,
//! and keeping the server's copy of the queue in step so any app can carry on. It does nothing
//! itself: what it needs done comes out of [`Player::take`], and answers go back in through its
//! methods.

use std::collections::HashMap;
use std::ops::Deref;
use std::time::{Duration, Instant};

use super::model::{Entry, Gain, Item, Repeat, Settings, Track};

/// Codecs the decoder reads; anything else comes from the server as FLAC.
pub const DECODES: [&str; 6] = ["flac", "mp3", "aac", "alac", "vorbis", "pcm"];

/// How long a fresh start waits for the server to measure a track's loudness.
const MEASURING: Duration = Duration::from_secs(4);

#[derive(Debug, Clone, PartialEq)]
pub struct QueueInput {
    pub tracks: Vec<i64>,
    pub current: usize,
    pub position: f64,
    pub shuffled: bool,
    pub repeat: Repeat,
}

#[derive(Debug, Clone, Default)]
pub struct PlayQueue {
    pub tracks: Vec<Track>,
    pub current: usize,
    pub position: f64,
    pub shuffled: bool,
    pub repeat: Repeat,
}

/// Another app playing (Feishin, Symfonium…), shown here without a sound.
#[derive(Debug, Clone, PartialEq)]
pub struct Remote {
    pub client: String,
    pub track: i64,
    pub paused: bool,
    /// Seconds in, as of `at`.
    pub position: f64,
    pub at: Instant,
}

/// In a listening room, controls go to the room, and the room drives the player.
#[derive(Debug, Clone, PartialEq)]
pub enum Room {
    Play(f64),
    Pause(f64),
    Seek(f64),
    Skip(usize),
    Add(Vec<i64>, bool),
    Remove(usize),
    Move(usize, usize),
}

/// Why the server's queue was asked for.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Ask {
    Restore,
    Refresh,
    Follow(u64),
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Timer {
    Save(u64),
    Measured(u64),
}

/// A change to the settings.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Change {
    Volume(f32),
    Muted(bool),
    Gain(Gain),
    Crossfade(f64),
}

/// What the player needs done.
#[derive(Debug, Clone, PartialEq)]
pub enum Do {
    Load(Vec<Item>, f64),
    Upcoming(String, Vec<Item>),
    Gain(String, f32),
    Play,
    Pause,
    Volume(f32),
    Stop,
    Save(QueueInput),
    Played(i64),
    NowPlaying(Option<i64>, f64, bool),
    /// Has the server measure a track's loudness; the answer goes to [`Player::measured`].
    Measure(i64),
    /// The server's queue; the answer goes to [`Player::queue`].
    Queue(Ask),
    /// A track by id, for following another app; the answer goes to [`Player::track`].
    Track(u64, i64),
    /// [`Player::timer`] after a while.
    After(Duration, Timer),
    /// A track began, or what's known about it changed.
    Started(String),
    Remember(Settings),
    Dismissed(bool),
    Room(Room),
}

#[derive(Debug, Clone, Default)]
pub struct State {
    pub queue: Vec<Entry>,
    pub index: usize,
    pub playing: bool,
    /// Waiting on the network or the decoder.
    pub waiting: bool,
    pub repeat: Repeat,
    pub shuffled: bool,
    /// The order before shuffling, to go back to.
    pub unshuffled: Option<Vec<Entry>>,
    /// Put away by its owner; the queue stays.
    pub dismissed: bool,
    /// Out of the way while a video plays.
    pub suspended: bool,
    /// Where it was left off, until playing starts again.
    pub resume_at: f64,
    pub error: Option<String>,
    pub settings: Settings,
    /// The listening room being followed, if any.
    pub room: Option<String>,
    /// Another app this player is mirroring, until playing here takes over.
    pub remote: Option<Remote>,
}
impl State {
    pub fn current(&self) -> Option<&Entry> {
        self.queue.get(self.index)
    }
}

/// What the engine last said it's playing.
struct Heard {
    key: String,
    time: f64,
    at: Instant,
}

/// A start waiting for a track's loudness to be measured.
struct Jump {
    ticket: u64,
    track: i64,
    play: bool,
    start: f64,
}

/// Following another app, while its queue or track is on the way.
struct Follow {
    ticket: u64,
    client: String,
    track: i64,
    time: f64,
    paused: bool,
}

pub struct Player {
    state: State,
    out: Vec<Do>,
    uid: u64,
    /// Whether sound was set up yet.
    sound: bool,
    heard: Option<Heard>,
    measuring: Vec<i64>,
    jump: Option<Jump>,
    jumps: u64,
    scrobbled: Option<String>,
    saving: u64,
    saved: Instant,
    restored: bool,
    /// The last each app said, so its regular check-ins aren't taken for a fresh start.
    told: HashMap<String, (i64, bool)>,
    following: u64,
    follow: Option<Follow>,
}
impl Deref for Player {
    type Target = State;

    fn deref(&self) -> &State {
        &self.state
    }
}
impl Default for Player {
    fn default() -> Self {
        Self::new(Settings::default(), false)
    }
}

/// How loud to play an entry: ReplayGain by the mode chosen, held back from clipping.
pub fn gain_of(i: usize, queue: &[Entry], settings: &Settings, shuffled: bool) -> f32 {
    let Some(e) = queue.get(i) else { return 1.0 };
    let g = &e.track.gains;
    let mut mode = settings.gain;
    if mode == Gain::Auto {
        // Album gain when an album plays in order, so its quiet songs stay quiet.
        let same = |o: Option<&Entry>| {
            o.is_some_and(|o| o.track.album_id.is_some() && o.track.album_id == e.track.album_id && !shuffled)
        };
        mode = if same(i.checked_sub(1).and_then(|i| queue.get(i))) || same(queue.get(i + 1)) {
            Gain::Album
        } else {
            Gain::Track
        };
    }
    if mode == Gain::Off {
        return 1.0;
    }
    let album = mode == Gain::Album;
    let db = if album { g.album_gain.or(g.track_gain) } else { g.track_gain }.unwrap_or(0.0);
    let peak = if album { g.album_peak.or(g.track_peak) } else { g.track_peak }.unwrap_or(0.0);
    let mut gain = 10f64.powf(db / 20.0);
    if peak > 0.0 && gain * peak > 1.0 {
        gain = 1.0 / peak;
    }
    gain as f32
}

/// One track after the other on the same album never crossfades; that's gapless.
pub fn fade_after(i: usize, queue: &[Entry], settings: &Settings) -> f64 {
    let (Some(a), Some(b)) = (queue.get(i), queue.get(i + 1)) else { return 0.0 };
    let (a, b) = (&a.track, &b.track);
    if settings.crossfade == 0.0 {
        return 0.0;
    }
    if a.album_id.is_some() && a.album_id == b.album_id && b.number.unwrap_or(0) == a.number.unwrap_or(0) + 1 {
        return 0.0;
    }
    settings.crossfade
}

pub fn shuffled<T>(mut list: Vec<T>) -> Vec<T> {
    for i in (1..list.len()).rev() {
        let mut bytes = [0; 8];
        if getrandom::fill(&mut bytes).is_ok() {
            list.swap(i, (u64::from_le_bytes(bytes) % (i as u64 + 1)) as usize);
        }
    }
    list
}

impl Player {
    pub fn new(settings: Settings, dismissed: bool) -> Self {
        Self {
            state: State { settings, dismissed, ..State::default() },
            out: vec![],
            uid: 0,
            sound: false,
            heard: None,
            measuring: vec![],
            jump: None,
            jumps: 0,
            scrobbled: None,
            saving: 0,
            saved: Instant::now(),
            restored: false,
            told: HashMap::new(),
            following: 0,
            follow: None,
        }
    }

    /// The state as it is, to set up pictures of the player in tests.
    #[cfg(test)]
    pub fn state_mut(&mut self) -> &mut State {
        &mut self.state
    }

    /// What's to be done since it was last asked.
    pub fn take(&mut self) -> Vec<Do> {
        std::mem::take(&mut self.out)
    }

    fn entry(&mut self, track: Track) -> Entry {
        self.uid += 1;
        Entry { key: format!("e{}", self.uid), track, fallback: false }
    }

    fn entries(&mut self, tracks: Vec<Track>) -> Vec<Entry> {
        tracks.into_iter().map(|t| self.entry(t)).collect()
    }

    fn item(&self, i: usize) -> Item {
        let s = &self.state;
        let e = &s.queue[i];
        let t = &e.track;
        let direct = !e.fallback && DECODES.contains(&t.codec.as_str());
        Item {
            key: e.key.clone(),
            path: if direct { t.file.clone() } else { t.flac.clone() },
            size: if direct { t.size } else { 0 },
            ext: if direct { t.suffix.clone() } else { "flac".into() },
            gain: gain_of(i, &s.queue, &s.settings, s.shuffled),
            crossfade: fade_after(i, &s.queue, &s.settings),
        }
    }

    /// The order the decoder should go through after `from`.
    fn upcoming(&self, from: usize) -> Vec<Item> {
        if self.repeat == Repeat::One {
            return vec![self.item(from)];
        }
        (from + 1..self.queue.len()).take(3).map(|i| self.item(i)).collect()
    }

    /// Tells the decoder what follows the current track, after the queue changed.
    fn line_up(&mut self) {
        let Some(e) = self.current() else { return };
        if !self.sound {
            return;
        }
        let key = e.key.clone();
        self.out.push(Do::Upcoming(key, self.upcoming(self.index)));
        self.measure_ahead();
    }

    /// Has the server measure a track's loudness if it hasn't yet.
    fn measure(&mut self, track: i64) {
        if !self.measuring.contains(&track) {
            self.measuring.push(track);
            self.out.push(Do::Measure(track));
        }
    }

    fn measure_ahead(&mut self) {
        if self.settings.gain == Gain::Off {
            return;
        }
        let pending: Vec<i64> = (self.index + 1..self.queue.len())
            .take(2)
            .filter(|&i| self.queue[i].track.gains.pending)
            .map(|i| self.queue[i].track.id)
            .collect();
        for id in pending {
            self.measure(id);
        }
    }

    /// The server measured a track, or couldn't (`None`): use what it found.
    pub fn measured(&mut self, id: i64, track: Option<Track>) {
        self.measuring.retain(|&t| t != id);
        if let Some(track) = track.filter(|t| t.id == id) {
            for e in &mut self.state.queue {
                if e.track.id == id {
                    e.track.gains = track.gains.clone();
                }
            }
            if self.sound {
                for i in 0..self.queue.len() {
                    if self.queue[i].track.id == id {
                        let gain = gain_of(i, &self.queue, &self.settings, self.shuffled);
                        self.out.push(Do::Gain(self.queue[i].key.clone(), gain));
                    }
                }
            }
        }
        if self.jump.as_ref().is_some_and(|j| j.track == id) {
            let jump = self.jump.take().unwrap();
            self.finish(jump.play, jump.start);
        }
    }

    pub fn timer(&mut self, timer: Timer) {
        match timer {
            Timer::Save(ticket) if ticket == self.saving => self.saved(),
            Timer::Measured(ticket) if self.jump.as_ref().is_some_and(|j| j.ticket == ticket) => {
                let jump = self.jump.take().unwrap();
                self.finish(jump.play, jump.start);
            },
            _ => {},
        }
    }

    /// Starts the entry at `i`, `at` seconds in.
    fn jump(&mut self, i: usize, play: bool, keep_position: bool, at: f64) {
        let Some(e) = self.queue.get(i) else { return };
        let (pending, id) = (e.track.gains.pending, e.track.id);
        let start = if keep_position { self.position().map_or(self.resume_at, |(_, t)| t) } else { at };
        let s = &mut self.state;
        s.index = i;
        s.error = None;
        s.waiting = play;
        s.resume_at = start;
        s.dismissed = false;
        s.remote = None;
        self.out.push(Do::Dismissed(false));
        self.sound = true;
        self.jumps += 1;
        self.jump = None;
        if pending && self.settings.gain != Gain::Off {
            // Measured just now, so the first second is already at the right level; it takes a
            // moment.
            self.jump = Some(Jump { ticket: self.jumps, track: id, play, start });
            self.measure(id);
            self.out.push(Do::After(MEASURING, Timer::Measured(self.jumps)));
            return;
        }
        self.finish(play, start);
    }

    fn finish(&mut self, play: bool, start: f64) {
        if self.current().is_none() {
            return;
        }
        let mut items = vec![self.item(self.index)];
        items.extend(self.upcoming(self.index));
        self.heard = Some(Heard { key: items[0].key.clone(), time: start, at: Instant::now() });
        self.out.push(Do::Load(items, start));
        self.out.push(Do::Volume(if self.settings.muted { 0.0 } else { self.settings.volume }));
        if play {
            self.out.push(Do::Play);
            self.state.playing = true;
        }
        self.measure_ahead();
        self.started();
        self.save(Some(Duration::from_millis(1500)));
    }

    fn started(&mut self) {
        let Some(e) = self.current() else { return };
        let (key, id) = (e.key.clone(), e.track.id);
        self.out.push(Do::Started(key));
        let time = self.position().map_or(0.0, |(_, t)| t);
        self.out.push(Do::NowPlaying(Some(id), time, !self.playing));
    }

    /// Where in the current track we are, in seconds.
    pub fn position(&self) -> Option<(String, f64)> {
        let e = self.current()?;
        if let Some(h) = self.heard.as_ref().filter(|h| h.key == e.key) {
            let ahead = if self.playing && !self.waiting { h.at.elapsed().as_secs_f64().min(0.5) } else { 0.0 };
            return Some((e.key.clone(), h.time + ahead));
        }
        let time = match &self.remote {
            Some(r) if !r.paused => (r.position + r.at.elapsed().as_secs_f64()).min(e.track.duration),
            _ => self.resume_at,
        };
        Some((e.key.clone(), time))
    }

    /// Where the current track is, for showing: where it'll pick up while paused.
    pub fn shown(&self) -> f64 {
        if self.playing || self.remote.as_ref().is_some_and(|r| !r.paused) {
            return self.position().map_or(0.0, |(_, t)| t);
        }
        if self.current().is_some() { self.resume_at } else { 0.0 }
    }

    /// Counts a play once enough of it was heard: half of it, or four minutes. Called every so
    /// often while playing.
    pub fn tick(&mut self) {
        let (Some(e), Some((_, time))) = (self.current(), self.position()) else { return };
        if !self.playing {
            return;
        }
        if self.scrobbled.as_deref() != Some(e.key.as_str()) && time >= (e.track.duration / 2.0).min(240.0) {
            let (key, id) = (e.key.clone(), e.track.id);
            self.scrobbled = Some(key);
            self.out.push(Do::Played(id));
        }
        if self.saved.elapsed() >= Duration::from_secs(30) {
            self.save(None);
        }
    }

    /// Writes the queue to the server after `soon`, or now, so other apps see it.
    fn save(&mut self, soon: Option<Duration>) {
        self.saving += 1;
        match soon {
            Some(soon) => self.out.push(Do::After(soon, Timer::Save(self.saving))),
            None => self.saved(),
        }
    }

    fn saved(&mut self) {
        self.saved = Instant::now();
        if (self.queue.is_empty() && !self.restored) || self.room.is_some() || self.remote.is_some() {
            return;
        }
        let input = QueueInput {
            tracks: self.queue.iter().map(|e| e.track.id).collect(),
            current: self.index,
            position: self.position().map_or(0.0, |(_, t)| t),
            shuffled: self.shuffled,
            repeat: self.repeat,
        };
        self.out.push(Do::Save(input));
    }

    /// Saves the queue now, before the app goes away.
    pub fn flush(&mut self) {
        if !self.queue.is_empty() {
            self.save(None);
        }
    }

    /// Picks up the queue where it was left, here or in another app; paused, as it was.
    pub fn restore(&mut self) {
        if self.restored {
            return;
        }
        self.restored = true;
        self.out.push(Do::Queue(Ask::Restore));
    }

    /// Another app changed the queue; take it over unless something's playing here.
    pub fn refresh_from_server(&mut self) {
        if !self.playing {
            self.out.push(Do::Queue(Ask::Refresh));
        }
    }

    /// Entries for these tracks, keeping the ones already in place so nothing's thrown away.
    fn keep(&mut self, tracks: Vec<Track>) -> Vec<Entry> {
        tracks
            .into_iter()
            .enumerate()
            .map(|(i, t)| match self.queue.get(i) {
                Some(e) if e.track.id == t.id => e.clone(),
                _ => self.entry(t),
            })
            .collect()
    }

    /// The server's queue arrived, or couldn't be had (`None`).
    pub fn queue(&mut self, ask: Ask, q: Option<PlayQueue>) {
        match ask {
            Ask::Restore => {
                let Some(q) = q.filter(|q| !q.tracks.is_empty()) else { return };
                if !self.queue.is_empty() {
                    return;
                }
                let queue = self.entries(q.tracks);
                let s = &mut self.state;
                s.index = q.current.min(queue.len() - 1);
                s.queue = queue;
                s.resume_at = q.position;
                s.repeat = q.repeat;
                s.shuffled = q.shuffled;
            },
            Ask::Refresh => {
                let Some(q) = q else { return };
                // While mirroring, stay on what the other app last said it's playing.
                let index = match &self.remote {
                    Some(r) => q.tracks.iter().position(|t| t.id == r.track),
                    None => (q.current < q.tracks.len()).then_some(q.current),
                };
                let Some(index) = index else { return };
                let queue = self.keep(q.tracks);
                let s = &mut self.state;
                if s.remote.is_none() {
                    s.resume_at = q.position;
                }
                s.queue = queue;
                s.index = index;
                s.repeat = q.repeat;
                s.shuffled = q.shuffled;
            },
            Ask::Follow(ticket) => {
                let Some(f) = self.follow.take_if(|f| f.ticket == ticket) else { return };
                if let Some(q) = q
                    && let Some(i) = q.tracks.iter().position(|t| t.id == f.track)
                {
                    let queue = self.keep(q.tracks);
                    self.mirror(f, queue, i);
                } else if let Some(i) = self.queue.iter().position(|e| e.track.id == f.track) {
                    let queue = self.queue.clone();
                    self.mirror(f, queue, i);
                } else {
                    self.out.push(Do::Track(ticket, f.track));
                    self.follow = Some(f);
                }
            },
        }
    }

    /// The track asked for to follow another app arrived, or couldn't be had.
    pub fn track(&mut self, ticket: u64, track: Option<Track>) {
        let Some(f) = self.follow.take_if(|f| f.ticket == ticket) else { return };
        let Some(track) = track else { return };
        let queue = vec![self.entry(track)];
        self.mirror(f, queue, 0);
    }

    /// Another app started, paused, moved or stopped: show the same track at the same spot without
    /// playing it. Playing here stops for a fresh start elsewhere, and pressing play here takes
    /// over.
    pub fn follow_remote(&mut self, client: &str, track: Option<i64>, time: f64, paused: bool) {
        let before = match track {
            Some(track) => self.told.insert(client.to_owned(), (track, paused)),
            None => self.told.remove(client),
        };
        if self.room.is_some() {
            return;
        }
        let Some(track) = track else {
            if self.remote.as_ref().is_some_and(|r| r.client == client) {
                self.state.resume_at = self.position().map_or(self.resume_at, |(_, t)| t);
                self.state.remote = None;
            }
            return;
        };
        let fresh = before.is_none_or(|(t, was)| t != track || (was && !paused));
        if self.playing && (paused || !fresh) {
            return;
        }
        self.following += 1;
        let f = Follow { ticket: self.following, client: client.to_owned(), track, time, paused };
        if self.current().is_some_and(|e| e.track.id == track) {
            let queue = self.queue.clone();
            self.mirror(f, queue, self.index);
        } else {
            self.out.push(Do::Queue(Ask::Follow(f.ticket)));
            self.follow = Some(f);
        }
    }

    fn mirror(&mut self, f: Follow, queue: Vec<Entry>, index: usize) {
        if f.ticket != self.following || self.room.is_some() {
            return;
        }
        if self.playing || self.heard.is_some() {
            self.out.push(Do::Stop);
            self.heard = None;
        }
        let s = &mut self.state;
        s.queue = queue;
        s.index = index;
        s.playing = false;
        s.waiting = false;
        s.error = None;
        s.dismissed = false;
        s.resume_at = f.time;
        s.remote =
            Some(Remote { client: f.client, track: f.track, paused: f.paused, position: f.time, at: Instant::now() });
        self.out.push(Do::Dismissed(false));
    }

    /// A track began playing.
    pub fn engine_track(&mut self, key: &str) {
        if self.heard.as_ref().is_none_or(|h| h.key != key) {
            self.heard = Some(Heard { key: key.to_owned(), time: 0.0, at: Instant::now() });
        }
        if let Some(i) = self.queue.iter().position(|e| e.key == key)
            && i != self.index
        {
            self.state.index = i;
            self.state.resume_at = 0.0;
            // The decoder only knows a few tracks ahead.
            self.line_up();
        }
        self.started();
    }

    /// Where in `key` the engine is.
    pub fn engine_position(&mut self, key: String, time: f64) {
        self.heard = Some(Heard { key, time, at: Instant::now() });
    }

    /// Everything queued has been played.
    pub fn engine_ended(&mut self) {
        if self.room.is_none() && self.repeat != Repeat::Off && !self.queue.is_empty() {
            let at = if self.repeat == Repeat::One { self.index } else { 0 };
            self.jump(at, true, false, 0.0);
        } else {
            self.state.playing = false;
            self.state.resume_at = 0.0;
            self.out.push(Do::Pause);
            self.save(Some(Duration::from_millis(1500)));
        }
    }

    pub fn engine_starved(&mut self, waiting: bool) {
        self.state.waiting = waiting;
    }

    pub fn engine_error(&mut self, key: &str, message: &str) {
        let Some(i) = self.queue.iter().position(|e| e.key == key) else { return };
        if !self.queue[i].fallback {
            // The decoder couldn't read it: get it from the server as FLAC and try again.
            self.state.queue[i].fallback = true;
            if i == self.index {
                self.jump(i, self.playing, true, 0.0);
            } else {
                self.line_up();
            }
            return;
        }
        tracing::warn!(message, "music");
        self.state.error = Some(format!("Can't play “{}”", self.queue[i].track.title));
    }

    /// The output went away or changed: carry on where it was.
    pub fn engine_reload(&mut self) {
        if self.current().is_some() && self.heard.is_some() {
            self.jump(self.index, self.playing, true, 0.0);
        }
    }

    /// Plays these tracks, from the one at `start`.
    pub fn play(&mut self, tracks: Vec<Track>, start: usize, shuffle: bool) {
        if tracks.is_empty() {
            return;
        }
        let mut queue = self.entries(tracks);
        let mut index = start.min(queue.len() - 1);
        let mut unshuffled = None;
        if shuffle {
            unshuffled = Some(queue.clone());
            queue = shuffled(queue);
            index = 0;
        }
        let s = &mut self.state;
        s.queue = queue;
        s.index = index;
        s.shuffled = shuffle;
        s.unshuffled = unshuffled;
        s.suspended = false;
        self.jump(index, true, false, 0.0);
    }

    pub fn toggle(&mut self) {
        if self.playing { self.pause() } else { self.resume() }
    }

    pub fn resume(&mut self) {
        if self.room.is_some() {
            let time = self.position().map_or(0.0, |(_, t)| t);
            return self.out.push(Do::Room(Room::Play(time)));
        }
        if self.current().is_none() {
            return;
        }
        self.sound = true;
        self.state.suspended = false;
        if self.heard.is_none() {
            let time = self.position().map_or(self.resume_at, |(_, t)| t);
            return self.jump(self.index, true, false, time);
        }
        self.out.push(Do::Play);
        self.state.playing = true;
        self.state.dismissed = false;
        self.started();
    }

    pub fn pause(&mut self) {
        if self.room.is_some() {
            let time = self.position().map_or(0.0, |(_, t)| t);
            return self.out.push(Do::Room(Room::Pause(time)));
        }
        if let Some(jump) = &mut self.jump {
            jump.play = false;
        }
        if !self.playing {
            return;
        }
        self.out.push(Do::Pause);
        self.state.resume_at = self.position().map_or(self.resume_at, |(_, t)| t);
        self.state.playing = false;
        let id = self.current().map(|e| e.track.id);
        self.out.push(Do::NowPlaying(id, self.resume_at, true));
        self.save(None);
    }

    pub fn seek(&mut self, time: f64) {
        if self.room.is_some() {
            return self.out.push(Do::Room(Room::Seek(time)));
        }
        let Some(e) = self.current() else { return };
        let t = time.min(e.track.duration - 0.25).max(0.0);
        if !self.sound {
            self.state.resume_at = t;
            self.state.remote = None;
            return;
        }
        self.jump(self.index, self.playing, false, t);
    }

    pub fn next(&mut self) {
        if self.room.is_some() {
            if self.index + 1 < self.queue.len() {
                self.out.push(Do::Room(Room::Skip(self.index + 1)));
            }
            return;
        }
        if self.index + 1 < self.queue.len() {
            self.jump(self.index + 1, true, false, 0.0);
        } else if self.repeat == Repeat::All && !self.queue.is_empty() {
            self.jump(0, true, false, 0.0);
        }
    }

    /// Back to the start of this one, or to the one before when it's only just begun.
    pub fn previous(&mut self) {
        let time = self.position().map_or(0.0, |(_, t)| t);
        if self.room.is_some() {
            let go = if time > 3.0 || self.index == 0 { Room::Seek(0.0) } else { Room::Skip(self.index - 1) };
            return self.out.push(Do::Room(go));
        }
        if time > 3.0 || self.index == 0 {
            self.seek(0.0);
        } else {
            self.jump(self.index - 1, true, false, 0.0);
        }
    }

    pub fn jump_to(&mut self, index: usize) {
        if self.room.is_some() {
            return self.out.push(Do::Room(Room::Skip(index)));
        }
        self.jump(index, true, false, 0.0);
    }

    /// Plays these right after the current track.
    pub fn play_next(&mut self, tracks: Vec<Track>) {
        if self.room.is_some() {
            return self.out.push(Do::Room(Room::Add(tracks.iter().map(|t| t.id).collect(), true)));
        }
        if self.queue.is_empty() {
            return self.play(tracks, 0, false);
        }
        let entries = self.entries(tracks);
        let at = self.index + 1;
        self.state.queue.splice(at..at, entries);
        self.line_up();
        self.save(Some(Duration::from_millis(1500)));
    }

    pub fn add(&mut self, tracks: Vec<Track>) {
        if self.room.is_some() {
            return self.out.push(Do::Room(Room::Add(tracks.iter().map(|t| t.id).collect(), false)));
        }
        if self.queue.is_empty() {
            return self.play(tracks, 0, false);
        }
        let entries = self.entries(tracks);
        self.state.queue.extend(entries);
        self.line_up();
        self.save(Some(Duration::from_millis(1500)));
    }

    pub fn remove(&mut self, key: &str) {
        let i = self.queue.iter().position(|e| e.key == key);
        if self.room.is_some() {
            if let Some(i) = i {
                self.out.push(Do::Room(Room::Remove(i)));
            }
            return;
        }
        let Some(i) = i else { return };
        self.state.queue.remove(i);
        if i == self.index {
            if self.queue.is_empty() {
                return self.clear();
            }
            self.state.index = i.min(self.queue.len() - 1);
            return self.jump(self.index, self.playing, false, 0.0);
        }
        if i < self.index {
            self.state.index -= 1;
        }
        self.line_up();
        self.save(Some(Duration::from_millis(1500)));
    }

    pub fn move_entry(&mut self, from: usize, to: usize) {
        if self.room.is_some() {
            return self.out.push(Do::Room(Room::Move(from, to)));
        }
        if from >= self.queue.len() || to >= self.queue.len() {
            return;
        }
        let playing = self.current().map(|e| e.key.clone());
        let e = self.state.queue.remove(from);
        self.state.queue.insert(to, e);
        self.state.index = self.queue.iter().position(|e| Some(&e.key) == playing.as_ref()).unwrap_or(0);
        self.line_up();
        self.save(Some(Duration::from_millis(1500)));
    }

    pub fn clear(&mut self) {
        self.out.push(Do::Stop);
        self.heard = None;
        let s = &mut self.state;
        s.queue.clear();
        s.index = 0;
        s.playing = false;
        s.resume_at = 0.0;
        s.unshuffled = None;
        s.shuffled = false;
        self.out.push(Do::NowPlaying(None, 0.0, true));
        self.save(None);
    }

    pub fn shuffle(&mut self, on: bool) {
        let Some(e) = self.current().map(|e| e.key.clone()) else { return };
        if on {
            let (current, rest): (Vec<Entry>, Vec<Entry>) = self.queue.iter().cloned().partition(|x| x.key == e);
            let s = &mut self.state;
            s.unshuffled = Some(std::mem::take(&mut s.queue));
            s.queue = current.into_iter().chain(shuffled(rest)).collect();
            s.index = 0;
            s.shuffled = true;
        } else {
            let s = &mut self.state;
            let has = |list: &[Entry], key: &str| list.iter().any(|x| x.key == key);
            let back: Vec<Entry> = match s.unshuffled.take() {
                Some(before) => before.into_iter().filter(|x| has(&s.queue, &x.key)).collect(),
                None => s.queue.clone(),
            };
            let extra: Vec<Entry> = s.queue.iter().filter(|x| !has(&back, &x.key)).cloned().collect();
            s.queue = back.into_iter().chain(extra).collect();
            s.index = s.queue.iter().position(|x| x.key == e).unwrap_or(0);
            s.shuffled = false;
        }
        self.line_up();
        self.save(Some(Duration::from_millis(1500)));
    }

    pub fn repeat(&mut self, mode: Repeat) {
        self.state.repeat = mode;
        self.line_up();
        self.save(Some(Duration::from_millis(1500)));
    }

    /// Puts the player away, pausing it first; the queue stays for later. A room plays on, so it
    /// stays.
    pub fn dismiss(&mut self) {
        if self.room.is_some() {
            return;
        }
        self.pause();
        self.state.dismissed = true;
        self.out.push(Do::Dismissed(true));
    }

    /// Steps aside for a video: pauses, and keeps out of sight until it's over.
    pub fn suspend(&mut self) {
        if self.playing {
            self.pause();
        }
        self.state.suspended = true;
    }

    pub fn unsuspend(&mut self) {
        self.state.suspended = false;
    }

    pub fn settings(&mut self, change: Change) {
        let s = &mut self.state.settings;
        match change {
            Change::Volume(volume) => s.volume = volume,
            Change::Muted(muted) => s.muted = muted,
            Change::Gain(gain) => s.gain = gain,
            Change::Crossfade(crossfade) => s.crossfade = crossfade,
        }
        self.out.push(Do::Remember(self.settings.clone()));
        match change {
            Change::Volume(_) | Change::Muted(_) if self.sound => {
                self.out.push(Do::Volume(if self.settings.muted { 0.0 } else { self.settings.volume }));
            },
            Change::Gain(_) => {
                if self.sound {
                    for i in 0..self.queue.len() {
                        let gain = gain_of(i, &self.queue, &self.settings, self.shuffled);
                        self.out.push(Do::Gain(self.queue[i].key.clone(), gain));
                    }
                }
                self.measure_ahead();
            },
            Change::Crossfade(_) => self.line_up(),
            _ => {},
        }
    }

    /// What's known about a track changed (it was starred, say).
    pub fn update(&mut self, track: &Track) {
        for e in &mut self.state.queue {
            if e.track.id == track.id {
                e.track = track.clone();
            }
        }
        if let Some(e) = self.current().filter(|e| e.track.id == track.id) {
            self.out.push(Do::Started(e.key.clone()));
        }
    }

    /// Hands the controls to a room; the personal queue waits on the server meanwhile.
    pub fn enter_room(&mut self, code: String) {
        if !self.queue.is_empty() && self.room.is_none() {
            self.save(None);
        }
        self.out.push(Do::Stop);
        self.heard = None;
        self.jump = None;
        let s = &mut self.state;
        s.room = Some(code);
        s.queue.clear();
        s.index = 0;
        s.playing = false;
        s.resume_at = 0.0;
        s.dismissed = false;
        s.suspended = false;
        s.remote = None;
    }

    /// Back to your own queue, paused where you left it.
    pub fn leave_room(&mut self) {
        self.out.push(Do::Stop);
        self.heard = None;
        self.jump = None;
        let s = &mut self.state;
        s.room = None;
        s.queue.clear();
        s.index = 0;
        s.playing = false;
        s.remote = None;
        self.restored = false;
        self.restore();
    }

    /// Makes the player match a room: its queue, the track it's on, and where in it. Small
    /// differences are left alone; the decoder keeps everyone gapless.
    pub fn follow(&mut self, tracks: Vec<Track>, index: usize, time: f64, playing: bool) {
        let same = tracks.len() == self.queue.len() && tracks.iter().zip(&self.queue).all(|(t, e)| t.id == e.track.id);
        if !same {
            // Keep entries that are still there, so the decoder's work isn't thrown away.
            self.state.queue = self.keep(tracks);
            if self.sound && self.index == index {
                self.line_up();
            }
        }
        let before =
            |n: usize, queue: &[Entry]| queue[..n.min(queue.len())].iter().map(|e| e.track.duration).sum::<f64>();
        let local = self.position().map_or(0.0, |(_, t)| before(self.index, &self.queue) + t);
        let target = before(index, &self.queue) + time;
        let off = !self.sound
            || self.heard.is_none()
            || self.index != index
            || (local - target).abs() > if playing { 0.35 } else { 1.0 };
        if off {
            return self.jump(index, playing, false, time);
        }
        if playing && !self.playing {
            self.out.push(Do::Play);
            self.state.playing = true;
        } else if !playing && self.playing {
            self.out.push(Do::Pause);
            self.state.playing = false;
            self.state.resume_at = time;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::model::Gains;
    use super::*;

    fn track(id: i64) -> Track {
        Track {
            id,
            title: format!("Track {id}"),
            artist: "Artist".into(),
            album: "Album".into(),
            album_id: Some(1),
            number: Some(id),
            duration: 200.0,
            codec: "flac".into(),
            suffix: "flac".into(),
            size: 1000,
            file: format!("/file/{id}"),
            flac: format!("/flac/{id}"),
            sample_rate: Some(44100),
            gains: Gains {
                track_gain: Some(-6.0),
                track_peak: Some(0.5),
                album_gain: Some(-3.0),
                album_peak: Some(0.9),
                pending: false,
            },
            ..Track::default()
        }
    }

    fn entries(tracks: Vec<Track>) -> Vec<Entry> {
        tracks
            .into_iter()
            .enumerate()
            .map(|(i, track)| Entry { key: format!("u{i}"), track, fallback: false })
            .collect()
    }

    fn close(a: f32, b: f64) -> bool {
        (a as f64 - b).abs() < 1e-5
    }

    fn loads(out: &[Do]) -> Vec<(Vec<Item>, f64)> {
        out.iter()
            .filter_map(|d| match d {
                Do::Load(items, start) => Some((items.clone(), *start)),
                _ => None,
            })
            .collect()
    }

    fn lined(out: &[Do]) -> Vec<(String, Vec<Item>)> {
        out.iter()
            .filter_map(|d| match d {
                Do::Upcoming(after, items) => Some((after.clone(), items.clone())),
                _ => None,
            })
            .collect()
    }

    fn paths(items: &[Item]) -> Vec<&str> {
        items.iter().map(|i| i.path.as_str()).collect()
    }

    fn server(queue: Vec<Track>) -> PlayQueue {
        PlayQueue { tracks: queue, current: 1, position: 42.0, shuffled: false, repeat: Repeat::Off }
    }

    #[test]
    fn album_gain_while_an_album_plays_in_order() {
        let mut other = track(3);
        other.album_id = Some(9);
        let q = entries(vec![track(1), track(2), other]);
        let settings = Settings::default();
        assert!(close(gain_of(0, &q, &settings, false), 10f64.powf(-3.0 / 20.0)));
        assert!(close(gain_of(2, &q, &settings, false), 10f64.powf(-6.0 / 20.0)));
        assert!(close(gain_of(0, &q, &settings, true), 10f64.powf(-6.0 / 20.0)));
        assert_eq!(gain_of(0, &q, &Settings { gain: Gain::Off, ..Settings::default() }, false), 1.0);
    }

    #[test]
    fn gain_never_clips() {
        let mut loud = track(1);
        loud.gains = Gains { track_gain: Some(6.0), track_peak: Some(0.9), ..Gains::default() };
        let q = entries(vec![loud]);
        assert!(close(gain_of(0, &q, &Settings { gain: Gain::Track, ..Settings::default() }, false), 1.0 / 0.9));
    }

    #[test]
    fn no_crossfade_from_one_track_of_an_album_into_the_next() {
        let settings = Settings { crossfade: 5.0, ..Settings::default() };
        let mut other = track(7);
        other.album_id = Some(2);
        let q = entries(vec![track(1), track(2), other, track(4)]);
        assert_eq!(fade_after(0, &q, &settings), 0.0);
        assert_eq!(fade_after(1, &q, &settings), 5.0);
        assert_eq!(fade_after(2, &q, &settings), 5.0);
        assert_eq!(fade_after(3, &q, &settings), 0.0);
        assert_eq!(fade_after(1, &q, &Settings::default()), 0.0);
    }

    #[test]
    fn plays_from_the_track_asked_for_with_what_comes_next_lined_up() {
        let mut player = Player::default();
        let mut wavpack = track(2);
        wavpack.codec = "wavpack".into();
        wavpack.suffix = "wv".into();
        player.play(vec![track(1), wavpack, track(3)], 0, false);
        let out = player.take();
        let (items, _) = &loads(&out)[0];
        assert_eq!(paths(items), ["/file/1", "/flac/2", "/file/3"]);
        assert_eq!((items[1].ext.as_str(), items[1].size), ("flac", 0));
        assert!(player.playing);
        assert!(out.contains(&Do::Play));
    }

    #[test]
    fn follows_the_engine_to_the_next_track() {
        let mut player = Player::default();
        player.play(vec![track(1), track(2)], 0, false);
        let key = loads(&player.take())[0].0[1].key.clone();
        player.engine_track(&key);
        assert_eq!(player.index, 1);
    }

    #[test]
    fn keeps_lining_up_as_each_track_starts() {
        let mut player = Player::default();
        player.play((1..=6).map(track).collect(), 0, false);
        let items = loads(&player.take())[0].0.clone();
        assert_eq!(items.len(), 4, "The decoder only gets a few tracks ahead");
        player.engine_track(&items[1].key);
        let (after, next) = lined(&player.take()).pop().unwrap();
        assert_eq!(after, items[1].key);
        assert_eq!(paths(&next), ["/file/3", "/file/4", "/file/5"]);
    }

    #[test]
    fn falls_back_to_flac_when_the_decoder_cant_read_a_file() {
        let mut player = Player::default();
        let mut mp3 = track(1);
        mp3.codec = "mp3".into();
        mp3.suffix = "mp3".into();
        player.play(vec![mp3], 0, false);
        let key = loads(&player.take())[0].0[0].key.clone();
        player.engine_error(&key, "bad");
        let (items, _) = loads(&player.take()).pop().unwrap();
        assert_eq!((items[0].path.as_str(), items[0].ext.as_str()), ("/flac/1", "flac"));
        player.engine_error(&items[0].key, "bad");
        assert!(player.error.as_deref().unwrap().contains("Track 1"));
    }

    #[test]
    fn lines_up_again_when_the_queue_changes() {
        let mut player = Player::default();
        player.play(vec![track(1), track(2)], 0, false);
        player.play_next(vec![track(5)]);
        let (_, items) = lined(&player.take()).pop().unwrap();
        assert_eq!(paths(&items), ["/file/5", "/file/2"]);
    }

    #[test]
    fn picks_up_the_queue_where_it_was_left_paused() {
        let mut player = Player::default();
        player.restore();
        assert_eq!(player.take(), [Do::Queue(Ask::Restore)]);
        player.queue(Ask::Restore, Some(server(vec![track(1), track(2)])));
        assert_eq!((player.index, player.resume_at, player.playing), (1, 42.0, false));
        player.resume();
        let (items, start) = loads(&player.take()).pop().unwrap();
        assert_eq!(start, 42.0);
        assert_eq!(items[0].path, "/file/2");
    }

    #[test]
    fn remembers_settings() {
        let mut player = Player::default();
        player.settings(Change::Crossfade(4.0));
        assert!(player.take().iter().any(|d| matches!(d, Do::Remember(s) if s.crossfade == 4.0)));
    }

    #[test]
    fn repeating_one_track_plays_it_again() {
        let mut player = Player::default();
        player.play(vec![track(1), track(2)], 0, false);
        player.repeat(Repeat::One);
        let (_, items) = lined(&player.take()).pop().unwrap();
        assert_eq!(paths(&items), ["/file/1"]);
        player.engine_ended();
        let (items, start) = loads(&player.take()).pop().unwrap();
        assert_eq!((items[0].path.as_str(), start), ("/file/1", 0.0));
        assert!(player.playing);
    }

    #[test]
    fn saves_the_queue_once_things_settle() {
        let mut player = Player::default();
        player.play(vec![track(1), track(2)], 1, false);
        let timers: Vec<Timer> = player
            .take()
            .into_iter()
            .filter_map(|d| match d {
                Do::After(_, timer @ Timer::Save(_)) => Some(timer),
                _ => None,
            })
            .collect();
        player.play_next(vec![track(3)]);
        player.take();
        player.timer(timers[0]);
        assert!(player.take().is_empty(), "A later change supersedes an earlier save");
        player.pause();
        let out = player.take();
        assert!(
            out.iter().any(|d| matches!(d, Do::Save(QueueInput { tracks, current: 1, .. }) if tracks == &[1, 2, 3]))
        );
        assert!(out.contains(&Do::NowPlaying(Some(2), 0.0, true)));
    }

    #[test]
    fn waits_a_moment_for_loudness_before_starting() {
        let mut player = Player::default();
        let mut fresh = track(1);
        fresh.gains = Gains { pending: true, ..Gains::default() };
        player.play(vec![fresh], 0, false);
        let out = player.take();
        assert!(out.contains(&Do::Measure(1)));
        assert!(loads(&out).is_empty());
        let mut measured = track(1);
        measured.gains.track_gain = Some(-6.0);
        player.measured(1, Some(measured));
        let (items, _) = loads(&player.take()).pop().unwrap();
        assert!(close(items[0].gain, 10f64.powf(-6.0 / 20.0)));
    }

    #[test]
    fn mirrors_another_app_without_playing() {
        let mut player = Player::default();
        player.follow_remote("Feishin", Some(2), 30.0, false);
        assert_eq!(player.take(), [Do::Queue(Ask::Follow(1))]);
        player.queue(Ask::Follow(1), Some(server(vec![track(1), track(2)])));
        assert_eq!(player.index, 1);
        assert!(!player.playing);
        assert!(player.remote.as_ref().is_some_and(|r| r.client == "Feishin"));
        assert!(player.shown() >= 30.0);
        player.follow_remote("Feishin", None, 0.0, true);
        assert!(player.remote.is_none());
    }

    #[test]
    fn controls_go_to_the_room() {
        let mut player = Player::default();
        player.enter_room("ROOM".into());
        player.follow(vec![track(1), track(2)], 1, 12.0, true);
        let (items, start) = loads(&player.take()).pop().unwrap();
        assert_eq!((items[0].path.as_str(), start), ("/file/2", 12.0));
        player.next();
        player.toggle();
        player.add(vec![track(3)]);
        let out = player.take();
        assert!(matches!(out[0], Do::Room(Room::Pause(_))));
        assert_eq!(out[1], Do::Room(Room::Add(vec![3], false)));
        player.follow(vec![track(1), track(2)], 1, 12.1, true);
        assert!(loads(&player.take()).is_empty(), "Small differences are left alone");
    }
}

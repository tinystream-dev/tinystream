// SPDX-License-Identifier: AGPL-3.0-or-later

//! What's kept from one server for offline use (#97): the files, each named after the server path
//! it came from, and `index.json`, which says what each download is made of and holds the
//! server's answers about it, so the app can ask again without a connection.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use ring::digest;
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Something downloaded.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(into = "String", try_from = "String")]
pub enum Key {
    Video(i64),
    Track(i64),
    Album(i64),
    Playlist(i64),
}

impl Key {
    /// Whether it's music, which an album or playlist can hold.
    pub fn is_track(self) -> bool {
        matches!(self, Self::Track(_))
    }
}

impl fmt::Display for Key {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Video(id) => write!(f, "video:{id}"),
            Self::Track(id) => write!(f, "track:{id}"),
            Self::Album(id) => write!(f, "album:{id}"),
            Self::Playlist(id) => write!(f, "playlist:{id}"),
        }
    }
}

impl From<Key> for String {
    fn from(key: Key) -> Self {
        key.to_string()
    }
}

impl TryFrom<String> for Key {
    type Error = String;

    fn try_from(text: String) -> Result<Self, String> {
        let (kind, id) = text.split_once(':').ok_or_else(|| format!("not a download: {text}"))?;
        let id = id.parse().map_err(|_| format!("not a download: {text}"))?;
        Ok(match kind {
            "video" => Self::Video(id),
            "track" => Self::Track(id),
            "album" => Self::Album(id),
            "playlist" => Self::Playlist(id),
            _ => return Err(format!("not a download: {text}")),
        })
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum State {
    #[default]
    Queued,
    Paused,
    Done,
    Failed,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Item {
    pub state: State,
    /// Why it failed.
    pub error: Option<String>,
    /// Downloaded on purpose. A track that isn't, and that no album or playlist holds, was only
    /// played lately, and goes first when the cache is full.
    pub kept: bool,
    /// The server paths of the files it needs, the one played first.
    pub files: Vec<String>,
    /// The server's answers to the app's questions about it, by [`answer_key`].
    pub answers: BTreeMap<String, Value>,
    /// What an album or playlist holds.
    pub parts: Vec<Key>,
    /// Bytes downloaded, and how many there are in all once that's known.
    pub size: u64,
    pub total: Option<u64>,
    /// Orders the queue: lower was asked for first.
    pub order: u64,
    /// When it was last played, in unix seconds.
    pub used: i64,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Index {
    pub items: BTreeMap<Key, Item>,
    /// Requests the server hasn't had yet (progress and plays made offline), oldest first.
    pub reports: Vec<Value>,
    next: u64,
}

impl Index {
    /// Asks for `key`, kept or only cached; one already there is kept if asked to be, and tried
    /// again if it failed. Whether it's new.
    pub fn add(&mut self, key: Key, kept: bool) -> bool {
        if let Some(item) = self.items.get_mut(&key) {
            item.kept |= kept;
            if item.state == State::Failed {
                item.state = State::Queued;
                item.error = None;
            }
            return false;
        }
        self.next += 1;
        self.items.insert(key, Item { kept, order: self.next, ..Item::default() });
        true
    }

    /// What to download next: whatever's waiting that was asked for first, with tracks about to
    /// play (which aren't kept) ahead of the rest.
    pub fn next_queued(&self) -> Option<Key> {
        self.items
            .iter()
            .filter(|(_, item)| item.state == State::Queued)
            .min_by_key(|(_, item)| (item.kept, item.order))
            .map(|(k, _)| *k)
    }

    /// Whether an album or playlist holds `key`, other than `except`.
    pub fn held(&self, key: Key, except: Option<Key>) -> bool {
        self.items.iter().any(|(k, item)| Some(*k) != except && item.parts.contains(&key))
    }

    /// Whether `key` stays when the cache is trimmed.
    pub fn pinned(&self, key: Key) -> bool {
        self.items.get(&key).is_some_and(|item| item.kept) || self.held(key, None)
    }

    /// The server's answer to `key` ([`answer_key`]), from whichever download has it.
    pub fn answer(&self, key: &str) -> Option<&Value> {
        self.items.values().find_map(|item| item.answers.get(key))
    }

    /// Takes `key` away, with the tracks of an album or playlist nothing else holds. The files no
    /// download needs anymore come back, to delete.
    pub fn remove(&mut self, key: Key) -> Vec<String> {
        let Some(item) = self.items.remove(&key) else { return Vec::new() };
        let mut files = item.files;
        for part in item.parts {
            if self.items.get(&part).is_some_and(|p| !p.kept) && !self.held(part, None) {
                files.extend(self.items.remove(&part).map(|p| p.files).unwrap_or_default());
            }
        }
        self.orphans(files)
    }

    /// Lets go of tracks that were only played lately, least recently played first, until they
    /// take up `limit` bytes or less. The files to delete come back.
    pub fn evict(&mut self, limit: u64) -> Vec<String> {
        let mut cached: Vec<(Key, i64, u64)> = self
            .items
            .iter()
            .filter(|(k, _)| k.is_track() && !self.pinned(**k))
            .map(|(k, item)| (*k, item.used, item.size))
            .collect();
        let mut total: u64 = cached.iter().map(|(_, _, size)| size).sum();
        cached.sort_by_key(|(_, used, _)| *used);
        let mut files = Vec::new();
        for (key, _, size) in cached {
            if total <= limit {
                break;
            }
            files.extend(self.items.remove(&key).map(|item| item.files).unwrap_or_default());
            total -= size;
        }
        self.orphans(files)
    }

    /// Of `files`, the ones no download lists anymore.
    fn orphans(&self, files: Vec<String>) -> Vec<String> {
        let used: BTreeSet<&str> = self.items.values().flat_map(|item| item.files.iter().map(String::as_str)).collect();
        let mut files: Vec<String> = files.into_iter().filter(|f| !used.contains(f.as_str())).collect();
        files.sort();
        files.dedup();
        files
    }
}

/// How an answer is found again: the operation's name and its variables.
pub fn answer_key(operation: &str, variables: &Value) -> String {
    format!("{operation} {variables}")
}

/// One server's downloads, in `root`.
pub struct Store {
    root: PathBuf,
    index: Mutex<Index>,
}

impl Store {
    pub fn open(root: PathBuf) -> Self {
        let index = std::fs::read(root.join("index.json"))
            .ok()
            .and_then(|bytes| serde_json::from_slice(&bytes).inspect_err(|e| tracing::warn!("offline index: {e}")).ok())
            .unwrap_or_default();
        Self { root, index: Mutex::new(index) }
    }

    pub fn read<T>(&self, read: impl FnOnce(&Index) -> T) -> T {
        read(&self.index.lock().unwrap_or_else(|e| e.into_inner()))
    }

    /// Changes the index and saves it.
    pub fn change<T>(&self, change: impl FnOnce(&mut Index) -> T) -> T {
        let mut index = self.index.lock().unwrap_or_else(|e| e.into_inner());
        let out = change(&mut index);
        if let Err(error) = self.write(&index) {
            tracing::warn!("can't save the offline index: {error}");
        }
        out
    }

    fn write(&self, index: &Index) -> std::io::Result<()> {
        std::fs::create_dir_all(&self.root)?;
        let temporary = self.root.join("index.json.tmp");
        std::fs::write(&temporary, serde_json::to_vec(index)?)?;
        std::fs::rename(temporary, self.root.join("index.json"))
    }

    /// Where the file from server path `path` is kept.
    pub fn file(&self, path: &str) -> PathBuf {
        self.root.join("files").join(name(path))
    }

    /// Where it goes while it downloads.
    pub fn part(&self, path: &str) -> PathBuf {
        self.file(path).with_extension("part")
    }

    /// The file from `path`, if it's all here.
    pub fn local(&self, path: &str) -> Option<PathBuf> {
        let file = self.file(path);
        file.is_file().then_some(file)
    }

    /// Removes `key` and the files nothing else needs.
    pub fn remove(&self, key: Key) {
        let files = self.change(|index| index.remove(key));
        self.delete(&files);
    }

    /// Trims the cache of recently played tracks to `limit` bytes.
    pub fn evict(&self, limit: u64) {
        let files = self.change(|index| index.evict(limit));
        self.delete(&files);
    }

    pub fn delete(&self, files: &[String]) {
        for path in files {
            let _ = std::fs::remove_file(self.file(path));
            let _ = std::fs::remove_file(self.part(path));
        }
    }
}

/// The name of the file from server path `path`: the same path with or without its leading slash.
fn name(path: &str) -> String {
    let hash = digest::digest(&digest::SHA256, path.trim_start_matches('/').as_bytes());
    hash.as_ref()[..16].iter().map(|b| format!("{b:02x}")).collect()
}

/// Bytes in the files under `dir`.
pub fn usage(dir: &Path) -> u64 {
    let Ok(entries) = std::fs::read_dir(dir) else { return 0 };
    entries
        .flatten()
        .map(|entry| match entry.file_type() {
            Ok(kind) if kind.is_dir() => usage(&entry.path()),
            Ok(_) => entry.metadata().map(|m| m.len()).unwrap_or(0),
            Err(_) => 0,
        })
        .sum()
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    fn done(index: &mut Index, key: Key, files: &[&str], size: u64, used: i64) {
        let item = index.items.get_mut(&key).unwrap();
        item.state = State::Done;
        item.files = files.iter().map(|f| (*f).to_owned()).collect();
        item.size = size;
        item.used = used;
    }

    #[test]
    fn keys_read_back() {
        for key in [Key::Video(12), Key::Track(-1), Key::Album(3), Key::Playlist(40)] {
            assert_eq!(Key::try_from(key.to_string()), Ok(key));
        }
        assert!(Key::try_from("film:2".to_owned()).is_err());
        assert_eq!(serde_json::to_value(Key::Album(3)).unwrap(), json!("album:3"));
    }

    #[test]
    fn the_queue_goes_in_the_order_asked_and_failures_are_retried() {
        let mut index = Index::default();
        assert!(index.add(Key::Video(9), true));
        assert!(index.add(Key::Video(2), true));
        assert_eq!(index.next_queued(), Some(Key::Video(9)));
        index.items.get_mut(&Key::Video(9)).unwrap().state = State::Paused;
        assert_eq!(index.next_queued(), Some(Key::Video(2)));

        index.add(Key::Track(5), false);
        assert_eq!(index.next_queued(), Some(Key::Track(5)));
        index.items.get_mut(&Key::Track(5)).unwrap().state = State::Done;

        let failed = index.items.get_mut(&Key::Video(2)).unwrap();
        failed.state = State::Failed;
        failed.error = Some("gone".into());
        assert_eq!(index.next_queued(), None);
        assert!(!index.add(Key::Video(2), true));
        assert_eq!(index.items[&Key::Video(2)].state, State::Queued);
        assert_eq!(index.items[&Key::Video(2)].error, None);
    }

    #[test]
    fn removing_an_album_keeps_tracks_kept_or_held_elsewhere() {
        let mut index = Index::default();
        index.add(Key::Album(1), true);
        index.add(Key::Playlist(1), true);
        for track in 1..=3 {
            index.add(Key::Track(track), false);
            done(&mut index, Key::Track(track), &[&format!("t{track}"), "cover"], 10, 0);
        }
        index.add(Key::Track(1), true);
        index.items.get_mut(&Key::Album(1)).unwrap().parts = vec![Key::Track(1), Key::Track(2), Key::Track(3)];
        index.items.get_mut(&Key::Album(1)).unwrap().files = vec!["cover".into()];
        index.items.get_mut(&Key::Playlist(1)).unwrap().parts = vec![Key::Track(2)];

        assert_eq!(index.remove(Key::Album(1)), vec!["t3".to_owned()]);
        assert!(index.items.contains_key(&Key::Track(1)));
        assert!(index.items.contains_key(&Key::Track(2)));
        assert!(!index.items.contains_key(&Key::Track(3)));

        assert_eq!(index.remove(Key::Playlist(1)), vec!["t2".to_owned()]);
        assert_eq!(index.remove(Key::Track(1)), vec!["cover".to_owned(), "t1".to_owned()]);
        assert!(index.items.is_empty());
    }

    #[test]
    fn the_cache_lets_go_of_the_least_recently_played_tracks_nobody_kept() {
        let mut index = Index::default();
        for (track, used) in [(1, 30), (2, 10), (3, 20), (4, 0)] {
            index.add(Key::Track(track), track == 4);
            done(&mut index, Key::Track(track), &[&format!("t{track}")], 100, used);
        }
        index.add(Key::Album(9), true);
        index.items.get_mut(&Key::Album(9)).unwrap().parts = vec![Key::Track(3)];

        assert_eq!(index.evict(250), Vec::<String>::new());
        assert_eq!(index.evict(150), vec!["t2".to_owned()]);
        assert_eq!(index.evict(0), vec!["t1".to_owned()]);
        assert!(index.items.contains_key(&Key::Track(3)));
        assert!(index.items.contains_key(&Key::Track(4)));
    }

    #[test]
    fn answers_and_files_survive_reopening() {
        let root = std::env::temp_dir().join(format!("tinystream-offline-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let store = Store::open(root.clone());
        let key = answer_key("Playback", &json!({"id": 4}));
        store.change(|index| {
            index.add(Key::Video(4), true);
            index.items.get_mut(&Key::Video(4)).unwrap().answers.insert(key.clone(), json!({"video": {"id": 4}}));
        });
        assert_eq!(store.file("/api/media/4/file"), store.file("api/media/4/file"));
        assert_eq!(store.local("api/media/4/file"), None);
        std::fs::create_dir_all(store.file("x").parent().unwrap()).unwrap();
        std::fs::write(store.file("api/media/4/file"), b"video").unwrap();
        assert_eq!(usage(&root), 5 + std::fs::metadata(root.join("index.json")).unwrap().len());

        let again = Store::open(root.clone());
        assert_eq!(again.read(|index| index.answer(&key).cloned()), Some(json!({"video": {"id": 4}})));
        assert_eq!(again.local("/api/media/4/file"), Some(store.file("api/media/4/file")));
        std::fs::remove_dir_all(root).unwrap();
    }
}

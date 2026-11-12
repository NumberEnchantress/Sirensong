//! The search log: with **Keep a search log** on, the results each search
//! showed are kept in `search-log.json` beside the play history, on this
//! computer only, and never uploaded.
//!
//! A search typed on in one go keeps one entry, its last: each new answer of
//! the same run replaces the one before, as the search history keeps one
//! line for it.

use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use serde::{Deserialize, Serialize};

use crate::api::models::{ArtistRef, SearchResults};

/// The searches the log keeps, newest first.
pub const KEEP: usize = 1000;

/// One search and what it showed.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Entry {
    /// When it was shown, as an RFC 3339 timestamp.
    pub at: String,
    /// The search as typed.
    pub query: String,
    pub results: Vec<Found>,
}

/// One result a search showed.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Found {
    /// `song`, `artist`, `album`, `playlist`, `podcast` or `episode`.
    pub kind: String,
    pub uri: String,
    pub name: String,
    /// The artists, for a song or an album.
    #[serde(skip_serializing_if = "String::is_empty")]
    pub by: String,
}

impl Entry {
    /// The results `results` shows for `query`, every kind in the order a
    /// search page lists them.
    pub fn of(query: &str, results: &SearchResults, at: String) -> Self {
        let by = |artists: &[ArtistRef]| {
            artists
                .iter()
                .map(|artist| artist.name.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        };
        let mut found = Vec::new();
        for track in results.tracks.iter().flat_map(|page| &page.items) {
            found.push(Found {
                kind: "song".into(),
                uri: track.uri.clone(),
                name: track.name.clone(),
                by: by(&track.artists),
            });
        }
        let mut plain = |kind: &str, uri: &str, name: &str| {
            found.push(Found {
                kind: kind.into(),
                uri: uri.into(),
                name: name.into(),
                by: String::new(),
            });
        };
        for artist in results.artists.iter().flat_map(|page| &page.items) {
            plain("artist", &artist.uri, &artist.name);
        }
        for playlist in results.playlists.iter().flat_map(|page| &page.items) {
            plain("playlist", &playlist.uri, &playlist.name);
        }
        for show in results.shows.iter().flat_map(|page| &page.items) {
            plain("podcast", &show.uri, &show.name);
        }
        for episode in results.episodes.iter().flat_map(|page| &page.items) {
            plain("episode", &episode.uri, &episode.name);
        }
        for album in results.albums.iter().flat_map(|page| &page.items) {
            found.push(Found {
                kind: "album".into(),
                uri: album.uri.clone(),
                name: album.name.clone(),
                by: by(&album.artists),
            });
        }
        Self {
            at,
            query: query.trim().to_string(),
            results: found,
        }
    }
}

/// The log, read from its file the first time it is needed.
#[derive(Default)]
pub struct SearchLog {
    entries: Vec<Entry>,
    read: bool,
    dirty: bool,
    /// The search run the newest entry came from, while it is still typed on.
    last_run: Option<u64>,
    /// When it last changed, so a search typed on is written once it rests.
    changed_at: Option<std::time::Instant>,
    failed: Arc<AtomicBool>,
}

/// How long the log rests after a change before it is written.
const SETTLE: std::time::Duration = std::time::Duration::from_secs(10);

impl SearchLog {
    /// Every search kept, newest first.
    pub fn entries(&mut self, path: &Path) -> &[Entry] {
        self.read(path);
        &self.entries
    }

    fn read(&mut self, path: &Path) {
        if std::mem::replace(&mut self.read, true) {
            return;
        }
        match std::fs::read_to_string(path) {
            Ok(text) => match serde_json::from_str::<Vec<Entry>>(&text) {
                Ok(entries) => self.entries = entries,
                Err(error) => log::warn!("could not read the search log: {error}"),
            },
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => log::warn!("could not read the search log: {error}"),
        }
    }

    /// Keeps `entry`, from search run `run`: in place of the newest entry
    /// when that one is the same run typed on, or newest otherwise.
    pub fn record(&mut self, path: &Path, entry: Entry, run: Option<u64>) {
        self.read(path);
        if entry.query.is_empty() {
            return;
        }
        let typed_on = run.is_some() && run == self.last_run && !self.entries.is_empty();
        if typed_on {
            self.entries[0] = entry;
        } else {
            self.entries.insert(0, entry);
            self.entries.truncate(KEEP);
        }
        self.last_run = run;
        self.dirty = true;
        self.changed_at = Some(std::time::Instant::now());
    }

    /// Forgets every search kept.
    pub fn clear(&mut self) {
        self.read = true;
        self.entries.clear();
        self.last_run = None;
        self.dirty = true;
        self.changed_at = Some(std::time::Instant::now());
    }

    /// Writes the log once it has rested since its last change.
    pub fn save_if_settled(&mut self, files: &fastframe_file::Writer, path: &Path) {
        if self.changed_at.is_some_and(|at| at.elapsed() >= SETTLE) {
            self.save(files, path);
        }
    }

    /// Writes the log if it changed, whole, off the UI thread, readable by
    /// its owner only.
    pub fn save(&mut self, files: &fastframe_file::Writer, path: &Path) {
        let failed = self.failed.swap(false, Ordering::SeqCst);
        if !std::mem::take(&mut self.dirty) && !failed {
            return;
        }
        self.changed_at = None;
        match serde_json::to_string(&self.entries) {
            Ok(text) => {
                let failed = self.failed.clone();
                files.write_or(path, text, fastframe_file::Access::Private, move || {
                    failed.store(true, Ordering::SeqCst)
                });
            }
            Err(error) => {
                self.dirty = true;
                log::warn!("could not write the search log: {error}");
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(query: &str) -> Entry {
        Entry {
            at: "2026-11-12T20:00:00Z".into(),
            query: query.into(),
            results: vec![Found {
                kind: "song".into(),
                uri: "spotify:track:a".into(),
                name: "A".into(),
                by: "B".into(),
            }],
        }
    }

    /// A search typed on keeps one entry, its last; another search adds one;
    /// the log keeps at most `KEEP` and reads back what it wrote.
    #[test]
    fn a_search_typed_on_keeps_one_entry() {
        let dir = std::env::temp_dir().join(format!("sirensong-search-log-{}", std::process::id()));
        let path = dir.join("search-log.json");
        let mut log = SearchLog::default();
        log.record(&path, entry("ab"), Some(1));
        log.record(&path, entry("abba"), Some(1));
        log.record(&path, entry("queen"), Some(2));
        let queries: Vec<_> = log
            .entries(&path)
            .iter()
            .map(|e| e.query.as_str())
            .collect();
        assert_eq!(queries, ["queen", "abba"]);
        for index in 0..KEEP + 5 {
            log.record(&path, entry(&format!("q{index}")), None);
        }
        assert_eq!(log.entries(&path).len(), KEEP);
        let text = serde_json::to_string(&log.entries).unwrap();
        let back: Vec<Entry> = serde_json::from_str(&text).unwrap();
        assert_eq!(back, log.entries);
        log.clear();
        assert!(log.entries(&path).is_empty());
    }
}

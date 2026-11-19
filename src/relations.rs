//! The playlist relation graph: playlists placed by the songs they share.
//!
//! Two playlists that share a song relate; the more they share, the closer
//! they sit. The weakest relation sets the unit: it stands one unit apart,
//! a stronger one proportionally closer, and playlists that share nothing
//! further apart than any relation. Positions come from stress
//! majorization (SMACOF) toward those distances, from a start fixed by each
//! playlist's key, so the same library always draws the same way.

use std::collections::{HashMap, HashSet, VecDeque};

use egui::{Vec2, vec2};

use crate::settings::RelationMode;

/// How far apart, in units of the weakest relation, two playlists that
/// share nothing are placed.
pub const APART: f32 = 1.6;

/// How many rounds of the layout to run.
const ROUNDS: usize = 300;

/// How strongly `a` and `b` relate under `mode`: the songs they share, or
/// the average of the shares of each that the other holds. Zero when they
/// share nothing, or either is empty.
pub fn strength(a: &HashSet<String>, b: &HashSet<String>, mode: RelationMode) -> f32 {
    if a.is_empty() || b.is_empty() {
        return 0.0;
    }
    let (small, large) = if a.len() <= b.len() { (a, b) } else { (b, a) };
    let shared = small.iter().filter(|song| large.contains(*song)).count() as f32;
    if shared == 0.0 {
        return 0.0;
    }
    match mode {
        RelationMode::Absolute => shared,
        RelationMode::Relative => (shared / a.len() as f32 + shared / b.len() as f32) / 2.0,
    }
}

/// Every pair's strength, `[i][j]`, symmetric, zero on the diagonal.
pub fn strengths(sets: &[&HashSet<String>], mode: RelationMode) -> Vec<Vec<f32>> {
    let count = sets.len();
    let mut table = vec![vec![0.0; count]; count];
    for i in 0..count {
        for j in i + 1..count {
            let value = strength(sets[i], sets[j], mode);
            table[i][j] = value;
            table[j][i] = value;
        }
    }
    table
}

/// The distance each pair is laid out toward, in `unit`s: the weakest
/// relation one unit, a relation `k` times as strong `1/k` of a unit, and
/// a pair that shares nothing [`APART`] units.
pub fn distances(table: &[Vec<f32>], unit: f32) -> Vec<Vec<f32>> {
    let weakest = table
        .iter()
        .flatten()
        .copied()
        .filter(|value| *value > 0.0)
        .fold(f32::INFINITY, f32::min);
    table
        .iter()
        .map(|row| {
            row.iter()
                .map(|value| {
                    if *value > 0.0 && weakest.is_finite() {
                        unit * weakest / value
                    } else {
                        unit * APART
                    }
                })
                .collect()
        })
        .collect()
}

/// A stable angle for a key, so a playlist starts in the same place.
fn start_angle(key: &str) -> f32 {
    use std::hash::{Hash, Hasher};
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    key.hash(&mut hasher);
    (hasher.finish() % 3600) as f32 / 3600.0 * std::f32::consts::TAU
}

/// Places for the playlists keyed `keys`, whose songs are `sets`, around
/// the origin: related ones toward their distances, the strongest
/// relations closest. The most related playlist sits at the origin.
pub fn layout(
    keys: &[&str],
    sets: &[&HashSet<String>],
    mode: RelationMode,
    unit: f32,
) -> Vec<Vec2> {
    let count = keys.len().min(sets.len());
    if count == 0 {
        return Vec::new();
    }
    let table = strengths(&sets[..count], mode);
    let target = distances(&table, unit);
    // Each pair weighs by the inverse square of its distance, as stress
    // majorization does, so near relations are kept most exactly.
    let mut at: Vec<Vec2> = keys[..count]
        .iter()
        .enumerate()
        .map(|(index, key)| {
            let angle = start_angle(key);
            let reach = unit * (0.5 + (index % 7) as f32 * 0.1);
            vec2(angle.cos(), angle.sin()) * reach
        })
        .collect();
    for _ in 0..ROUNDS {
        let mut next = at.clone();
        for i in 0..count {
            let mut sum = Vec2::ZERO;
            let mut weights = 0.0;
            for j in 0..count {
                if i == j {
                    continue;
                }
                let d = target[i][j];
                let weight = 1.0 / (d * d);
                let apart = at[i] - at[j];
                let length = apart.length();
                let away = if length > 1e-4 {
                    apart / length
                } else {
                    let angle = start_angle(keys[i]) - start_angle(keys[j]);
                    vec2(angle.cos(), angle.sin())
                };
                sum += (at[j] + away * d) * weight;
                weights += weight;
            }
            if weights > 0.0 {
                next[i] = sum / weights;
            }
        }
        at = next;
    }
    // The most related playlist at the middle.
    let hub = (0..count)
        .max_by(|a, b| {
            let total = |i: usize| table[i].iter().filter(|value| **value > 0.0).count();
            total(*a).cmp(&total(*b)).then(b.cmp(a))
        })
        .unwrap_or(0);
    let middle = at[hub];
    at.iter().map(|place| *place - middle).collect()
}

/// The songs of the playlists the relation graph shows, read a page at a
/// time, one playlist after another, while the graph is open. Kept for the
/// run: a playlist is read again only when its song count changes.
#[derive(Default)]
pub struct RelationSongs {
    /// Each playlist's songs, by id, once read whole, with the song count
    /// they were read at.
    pub songs: HashMap<String, (u32, HashSet<String>)>,
    /// What is being read: the playlist, its song count, the songs so far,
    /// and where its next page starts.
    reading: Option<(String, u32, HashSet<String>, u32)>,
    /// The page asked for and not yet answered.
    pub asked: Option<(String, u32)>,
    /// Playlists still to read, in order.
    queue: VecDeque<(String, u32)>,
    /// Playlists that could not be read, not asked again this run.
    pub failed: HashSet<String>,
}

impl RelationSongs {
    /// Ask for the songs of `wanted`, each with its song count: what is
    /// already read at that count stays, the rest queue in order.
    pub fn want(&mut self, wanted: &[(String, u32)]) {
        for (id, total) in wanted {
            let known = self
                .songs
                .get(id)
                .is_some_and(|(read_at, _)| read_at == total);
            let reading = self
                .reading
                .as_ref()
                .is_some_and(|(held, _, _, _)| held == id);
            let queued = self.queue.iter().any(|(held, _)| held == id);
            if !known && !reading && !queued && !self.failed.contains(id) {
                self.queue.push_back((id.clone(), *total));
            }
        }
    }

    /// The next page to ask for, if none is waiting: the next of the
    /// playlist being read, or the first of the next one queued.
    pub fn next_page(&mut self) -> Option<(String, u32)> {
        if self.asked.is_some() {
            return None;
        }
        if self.reading.is_none() {
            let (id, total) = self.queue.pop_front()?;
            self.reading = Some((id, total, HashSet::new(), 0));
        }
        let (id, _, _, offset) = self.reading.as_ref()?;
        let page = (id.clone(), *offset);
        self.asked = Some(page.clone());
        Some(page)
    }

    /// A page of `id` from `offset`: its songs' URIs, and the offset of the
    /// next page, if any. A playlist read to its end joins [`Self::songs`].
    pub fn receive(&mut self, id: &str, offset: u32, uris: Vec<String>, next: Option<u32>) {
        if self.asked.as_ref() != Some(&(id.to_string(), offset)) {
            return;
        }
        self.asked = None;
        let Some((reading, _, read, at)) = self.reading.as_mut() else {
            return;
        };
        if reading != id {
            return;
        }
        read.extend(uris);
        // The read ends at the last page, or at a next page that would not
        // move on, so a playlist cannot keep it asking.
        match next.filter(|next| *next > offset) {
            Some(next) => *at = next,
            None => {
                if let Some((id, total, read, _)) = self.reading.take() {
                    self.songs.insert(id, (total, read));
                }
            }
        }
    }

    /// `id` could not be read: leave it out of the graph for this run.
    pub fn fail(&mut self, id: &str) {
        if self.asked.as_ref().is_some_and(|(asked, _)| asked == id) {
            self.asked = None;
        }
        if self
            .reading
            .as_ref()
            .is_some_and(|(held, _, _, _)| held == id)
        {
            self.reading = None;
        }
        self.failed.insert(id.to_string());
    }

    /// How many of `wanted` are read whole.
    pub fn read_count(&self, wanted: &[(String, u32)]) -> usize {
        wanted
            .iter()
            .filter(|(id, _)| self.songs.contains_key(id))
            .count()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn set(songs: &[&str]) -> HashSet<String> {
        songs.iter().map(|song| song.to_string()).collect()
    }

    /// Absolute counts the shared songs; relative averages the share of
    /// each playlist the other holds.
    #[test]
    fn strength_counts_or_averages_the_shared_songs() {
        let a = set(&["1", "2", "3", "4"]);
        let b = set(&["3", "4"]);
        let c = set(&["9"]);
        assert_eq!(strength(&a, &b, RelationMode::Absolute), 2.0);
        assert_eq!(strength(&a, &b, RelationMode::Relative), (0.5 + 1.0) / 2.0);
        assert_eq!(strength(&a, &c, RelationMode::Absolute), 0.0);
        assert_eq!(strength(&a, &set(&[]), RelationMode::Relative), 0.0);
    }

    /// The weakest relation is a unit apart, a stronger one closer by its
    /// strength, and an unrelated pair further than any relation.
    #[test]
    fn distances_follow_the_weakest_relation() {
        let table = vec![
            vec![0.0, 2.0, 1.0, 0.0],
            vec![2.0, 0.0, 0.0, 0.0],
            vec![1.0, 0.0, 0.0, 0.0],
            vec![0.0, 0.0, 0.0, 0.0],
        ];
        let target = distances(&table, 10.0);
        assert_eq!(target[0][2], 10.0, "the weakest is one unit");
        assert_eq!(target[0][1], 5.0, "twice as strong, half as far");
        assert_eq!(target[0][3], 10.0 * APART, "unrelated beyond");
    }

    /// The layout keeps strong relations closer than weak ones, and
    /// unrelated playlists furthest; it is the same every time.
    #[test]
    fn the_layout_places_closer_what_shares_more() {
        let a = set(&["1", "2", "3", "4", "5", "6"]);
        let b = set(&["1", "2", "3", "4", "5", "7"]);
        let c = set(&["1", "8", "9"]);
        let d = set(&["x", "y"]);
        let keys = ["a", "b", "c", "d"];
        let sets = [&a, &b, &c, &d];
        let at = layout(&keys, &sets, RelationMode::Absolute, 100.0);
        let gap = |i: usize, j: usize| (at[i] - at[j]).length();
        assert!(gap(0, 1) < gap(0, 2), "five shared sit closer than one");
        assert!(gap(0, 2) < gap(0, 3), "a relation closer than none");
        assert_eq!(at, layout(&keys, &sets, RelationMode::Absolute, 100.0));
    }

    /// Songs are read one playlist and one page at a time; a playlist read
    /// whole is kept until its song count changes; one that fails is left.
    #[test]
    fn songs_are_read_a_page_at_a_time() {
        let mut songs = RelationSongs::default();
        let wanted = vec![("p1".to_string(), 3), ("p2".to_string(), 1)];
        songs.want(&wanted);
        assert_eq!(songs.next_page(), Some(("p1".to_string(), 0)));
        assert_eq!(songs.next_page(), None, "one page at a time");
        songs.receive("p1", 0, vec!["a".into(), "b".into()], Some(2));
        assert_eq!(songs.next_page(), Some(("p1".to_string(), 2)));
        songs.receive("p1", 2, vec!["c".into()], None);
        assert_eq!(songs.read_count(&wanted), 1);
        assert_eq!(songs.next_page(), Some(("p2".to_string(), 0)));
        songs.fail("p2");
        songs.want(&wanted);
        assert_eq!(
            songs.next_page(),
            None,
            "read and failed are not asked again"
        );
        songs.want(&[("p1".to_string(), 4)]);
        assert_eq!(
            songs.next_page(),
            Some(("p1".to_string(), 0)),
            "a changed count reads it again"
        );
    }
}

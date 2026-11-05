//! Sirensong custom order: the Library as the listener arranges it, one
//! tree that holds anything at any depth. It uses the rootlist's own rows,
//! a folder's start and end around what it holds, so the folder edits and
//! their checks in `crate::folders` work on it unchanged. An item is any
//! Library key: a playlist, album, artist or podcast URI, Liked Songs or
//! Local Files.

use std::collections::HashSet;

use serde::{Deserialize, Serialize};

use crate::player::RootlistEntry;
use crate::settings::LibraryShelf;

/// The key Local Files is arranged by.
pub const LOCAL_FILES_KEY: &str = "spotifast:local-files";

/// One account's custom order.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct LibraryTree {
    /// The tree, in the rootlist's rows.
    pub rows: Vec<RootlistEntry>,
    /// The kinds it has placed; a kind seen for the first time goes at the
    /// end, what is saved later on top.
    pub kinds: Vec<LibraryShelf>,
    /// Changed by the listener since it was made. Until then, a first tree
    /// made before Spotify's folders were known gives way to one made from
    /// them.
    pub edited: bool,
    /// Spotify's tree as it was when the two last agreed, kept to tell
    /// which side changed since. `None` until they first agree.
    pub base: Option<Vec<RootlistEntry>>,
}

/// The kind an item is, by its key.
pub fn kind_of(key: &str) -> LibraryShelf {
    if key.starts_with("spotify:album:") {
        LibraryShelf::Albums
    } else if key.starts_with("spotify:artist:") {
        LibraryShelf::Artists
    } else if key.starts_with("spotify:show:") {
        LibraryShelf::Podcasts
    } else {
        LibraryShelf::Playlists
    }
}

/// The keys the tree places.
pub fn keys(tree: &[RootlistEntry]) -> HashSet<&str> {
    tree.iter()
        .filter_map(|row| match row {
            RootlistEntry::Playlist(key) => Some(key.as_str()),
            _ => None,
        })
        .collect()
}

/// The first tree: `top` (the pins and Liked Songs) first, then Spotify's
/// tree as it is, or, before Spotify's tree is known, the flat order this
/// computer kept. A pinned folder rises with everything in it.
pub fn seed(top: &[String], spotify: &[RootlistEntry], flat: &[String]) -> Vec<RootlistEntry> {
    let mut rest: Vec<RootlistEntry> = if spotify.is_empty() {
        flat.iter().cloned().map(RootlistEntry::Playlist).collect()
    } else {
        spotify.to_vec()
    };
    let mut tree = Vec::new();
    let mut placed = HashSet::new();
    for key in top {
        if !placed.insert(key.clone()) {
            continue;
        }
        if let Some(id) = crate::pins::folder_id(key) {
            if let Some((start, end)) = crate::folders::folder_span(&rest, id) {
                tree.extend(rest.drain(start..=end));
            }
            continue;
        }
        tree.push(RootlistEntry::Playlist(key.clone()));
    }
    tree.extend(rest);
    // Each item once: a pin lifted to the top leaves its old place, and a
    // pin inside a pinned folder stays in it.
    let mut seen = HashSet::new();
    tree.retain(|row| match row {
        RootlistEntry::Playlist(key) => seen.insert(key.clone()),
        _ => true,
    });
    tree
}

/// Where a new save goes: the top, below Liked Songs when it leads, as
/// Spotify keeps Liked Songs pinned.
pub fn top(tree: &[RootlistEntry]) -> usize {
    usize::from(matches!(
        tree.first(),
        Some(RootlistEntry::Playlist(key)) if key == crate::settings::LIKED_SONGS_KEY
    ))
}

/// Places items the tree has not met: `new` at the top, newest first, as
/// Spotify shows a new save, and `first_seen` (a kind seen for the first
/// time) at the end, in their own order.
pub fn place(tree: &mut Vec<RootlistEntry>, new: &[String], first_seen: &[String]) -> bool {
    let known: HashSet<String> = keys(tree).into_iter().map(str::to_owned).collect();
    let mut added = HashSet::new();
    let top: Vec<RootlistEntry> = new
        .iter()
        .filter(|key| !known.contains(*key) && added.insert((*key).clone()))
        .cloned()
        .map(RootlistEntry::Playlist)
        .collect();
    let end: Vec<RootlistEntry> = first_seen
        .iter()
        .filter(|key| !known.contains(*key) && added.insert((*key).clone()))
        .cloned()
        .map(RootlistEntry::Playlist)
        .collect();
    if top.is_empty() && end.is_empty() {
        return false;
    }
    let at = self::top(tree);
    tree.splice(at..at, top);
    tree.extend(end);
    true
}

/// Whether Spotify's tree can hold the item: playlists only.
pub fn on_spotify(key: &str) -> bool {
    key.starts_with("spotify:playlist:")
}

/// What Spotify's tree would hold of `rows`: the folders and playlists, in
/// their places, without what only Sirensong arranges.
pub fn projection(rows: &[RootlistEntry]) -> Vec<RootlistEntry> {
    rows.iter()
        .filter(|row| match row {
            RootlistEntry::Playlist(key) => on_spotify(key),
            _ => true,
        })
        .cloned()
        .collect()
}

/// A row the two trees share, by which the other rows are placed.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
enum Anchor {
    /// The start of a folder, or of the tree for `None`.
    Start(Option<String>),
    /// Just after a playlist.
    AfterItem(String),
    /// Just after a folder and everything in it.
    AfterFolder(String),
}

/// Spotify's tree, with what only Sirensong arranges put back where it
/// was: each such row stays after the shared row it followed, in the same
/// folder, or at the start of that folder. A folder Spotify lacks comes back
/// whole when it is new here (not in `base`, Spotify's tree when the two
/// last agreed) and holds nothing of Spotify's; otherwise Spotify removed
/// it, so what else it held stays one level up. A playlist Spotify's tree
/// no longer holds is gone from the library, so it goes from here too.
pub fn merge(
    local: &[RootlistEntry],
    spotify: &[RootlistEntry],
    base: &[RootlistEntry],
) -> Vec<RootlistEntry> {
    let known = |id: &str| {
        base.iter()
            .any(|row| matches!(row, RootlistEntry::FolderStart { id: held, .. } if held == id))
    };
    let items: HashSet<&str> = keys(spotify);
    let folders: HashSet<&str> = spotify
        .iter()
        .filter_map(|row| match row {
            RootlistEntry::FolderStart { id, .. } => Some(id.as_str()),
            _ => None,
        })
        .collect();
    let shared = |row: &RootlistEntry| match row {
        RootlistEntry::Playlist(key) => items.contains(key.as_str()),
        RootlistEntry::FolderStart { id, .. } => folders.contains(id.as_str()),
        RootlistEntry::FolderEnd => false,
    };
    let mut followers: std::collections::HashMap<Anchor, Vec<RootlistEntry>> = Default::default();
    // The enclosing folders, each with whether Spotify has it.
    let mut parents: Vec<(String, bool)> = Vec::new();
    let mut last: std::collections::HashMap<Option<String>, Anchor> = Default::default();
    let effective = |parents: &[(String, bool)]| {
        parents
            .iter()
            .rev()
            .find(|(_, on)| *on)
            .map(|(id, _)| id.clone())
    };
    let mut index = 0;
    while index < local.len() {
        let row = &local[index];
        let parent = effective(&parents);
        let anchor = last
            .get(&parent)
            .cloned()
            .unwrap_or(Anchor::Start(parent.clone()));
        match row {
            RootlistEntry::FolderStart { id, .. } if shared(row) => {
                parents.push((id.clone(), true));
                index += 1;
            }
            RootlistEntry::FolderStart { id, .. } => {
                let (_, end) = crate::folders::folder_span(local, id).unwrap_or((index, index));
                let span = &local[index..=end];
                if known(id) || span.iter().any(&shared) {
                    // Gone from Spotify: its own rows stay, one level up.
                    parents.push((id.clone(), false));
                    index += 1;
                } else {
                    followers.entry(anchor).or_default().extend(
                        span.iter()
                            .filter(|row| {
                                !matches!(row, RootlistEntry::Playlist(key) if on_spotify(key))
                            })
                            .cloned(),
                    );
                    index = end + 1;
                }
            }
            RootlistEntry::FolderEnd => {
                if let Some((id, on)) = parents.pop()
                    && on
                {
                    last.insert(effective(&parents), Anchor::AfterFolder(id));
                }
                index += 1;
            }
            RootlistEntry::Playlist(key) => {
                if shared(row) {
                    last.insert(parent, Anchor::AfterItem(key.clone()));
                } else if !on_spotify(key) {
                    followers.entry(anchor).or_default().push(row.clone());
                }
                index += 1;
            }
        }
    }
    let mut out = followers.remove(&Anchor::Start(None)).unwrap_or_default();
    let mut open: Vec<String> = Vec::new();
    for row in spotify {
        out.push(row.clone());
        match row {
            RootlistEntry::FolderStart { id, .. } => {
                open.push(id.clone());
                out.extend(
                    followers
                        .remove(&Anchor::Start(Some(id.clone())))
                        .unwrap_or_default(),
                );
            }
            RootlistEntry::FolderEnd => {
                if let Some(id) = open.pop() {
                    out.extend(
                        followers
                            .remove(&Anchor::AfterFolder(id))
                            .unwrap_or_default(),
                    );
                }
            }
            RootlistEntry::Playlist(key) => {
                out.extend(
                    followers
                        .remove(&Anchor::AfterItem(key.clone()))
                        .unwrap_or_default(),
                );
            }
        }
    }
    // Nothing is lost: anything whose place is gone goes on top.
    let rest: Vec<RootlistEntry> = followers.into_values().flatten().collect();
    out.splice(0..0, rest);
    let mut seen = HashSet::new();
    out.retain(|row| match row {
        RootlistEntry::Playlist(key) => seen.insert(key.clone()),
        _ => true,
    });
    out
}

/// Where `row` stands in `target`, as a place `tree` (Spotify's) knows:
/// before its next sibling that `tree` holds, or at the end of its folder.
pub fn spotify_place(
    target: &[RootlistEntry],
    row: &crate::folders::Row,
    tree: &[RootlistEntry],
) -> Option<crate::folders::Place> {
    use crate::folders::{Place, Row};
    let start = position(target, row)?;
    let end = match row {
        Row::Folder(id) => crate::folders::folder_span(target, id)?.1,
        Row::Playlist(_) => start,
    };
    let in_tree = |row: &Row| position(tree, row).is_some();
    let mut index = end + 1;
    let mut previous: Option<String> = None;
    // The siblings before it, for a row that ends its folder.
    let parent = crate::folders::parent(target, row);
    for at in (0..start).rev() {
        match &target[at] {
            RootlistEntry::Playlist(key)
                if crate::folders::parent(target, &Row::Playlist(key.clone())) == parent =>
            {
                if in_tree(&Row::Playlist(key.clone())) {
                    previous = Some(key.clone());
                }
                break;
            }
            RootlistEntry::FolderStart { id, .. } if Some(id) == parent.as_ref() => break,
            _ => {}
        }
    }
    while index < target.len() {
        match &target[index] {
            RootlistEntry::FolderEnd => break,
            RootlistEntry::Playlist(key) => {
                let next = Row::Playlist(key.clone());
                if in_tree(&next) {
                    return Some(Place::Before(next));
                }
                index += 1;
            }
            RootlistEntry::FolderStart { id, .. } => {
                let next = Row::Folder(id.clone());
                if in_tree(&next) {
                    return Some(Place::Before(next));
                }
                index = crate::folders::folder_span(target, id)?.1 + 1;
            }
        }
    }
    Some(match (parent, previous) {
        (_, Some(key)) => Place::AfterPlaylist(key),
        (Some(id), None) if in_tree(&Row::Folder(id.clone())) => Place::InFolder(id),
        _ => Place::End,
    })
}

/// Where `row` is in `rows`.
fn position(rows: &[RootlistEntry], row: &crate::folders::Row) -> Option<usize> {
    use crate::folders::Row;
    rows.iter().position(|held| match (held, row) {
        (RootlistEntry::Playlist(key), Row::Playlist(wanted)) => key == wanted,
        (RootlistEntry::FolderStart { id, .. }, Row::Folder(wanted)) => id == wanted,
        _ => false,
    })
}

/// The row after `row` in its folder, if any, and its folder.
fn neighbour(
    rows: &[RootlistEntry],
    row: &crate::folders::Row,
) -> (Option<String>, Option<crate::folders::Row>) {
    use crate::folders::Row;
    let parent = crate::folders::parent(rows, row);
    let Some(start) = position(rows, row) else {
        return (parent, None);
    };
    let end = match row {
        Row::Folder(id) => crate::folders::folder_span(rows, id).map_or(start, |(_, end)| end),
        Row::Playlist(_) => start,
    };
    let next = match rows.get(end + 1) {
        Some(RootlistEntry::Playlist(key)) => Some(Row::Playlist(key.clone())),
        Some(RootlistEntry::FolderStart { id, .. }) => Some(Row::Folder(id.clone())),
        _ => None,
    };
    (parent, next)
}

/// The edits that turn `spotify` into `target`, in order: new folders,
/// renamed ones, each row whose next sibling changed put back before it,
/// last row first, and folders `target` no longer has removed. Each edit
/// is checked on a copy, and one that would change nothing is left out.
pub fn push_edits(
    spotify: &[RootlistEntry],
    target: &[RootlistEntry],
) -> Vec<crate::folders::FolderEdit> {
    use crate::folders::{FolderEdit, Place, Row};
    let mut tree = spotify.to_vec();
    let mut edits = Vec::new();
    let mut make = |tree: &mut Vec<RootlistEntry>, edit: FolderEdit| {
        if let Ok(next) = crate::folders::apply(tree, &edit) {
            *tree = next;
            edits.push(edit);
        }
    };
    let names = |rows: &[RootlistEntry]| -> std::collections::HashMap<String, String> {
        rows.iter()
            .filter_map(|row| match row {
                RootlistEntry::FolderStart { id, name } => Some((id.clone(), name.clone())),
                _ => None,
            })
            .collect()
    };
    let wanted = names(target);
    let held = names(&tree);
    for row in target {
        if let RootlistEntry::FolderStart { id, name } = row {
            match held.get(id) {
                None => make(
                    &mut tree,
                    FolderEdit::Create {
                        id: id.clone(),
                        name: name.clone(),
                        place: Place::Start,
                        adopt: None,
                    },
                ),
                Some(old) if old != name => make(
                    &mut tree,
                    FolderEdit::Rename {
                        id: id.clone(),
                        name: name.clone(),
                    },
                ),
                Some(_) => {}
            }
        }
    }
    let rows: Vec<Row> = target
        .iter()
        .filter_map(|row| match row {
            RootlistEntry::Playlist(key) => Some(Row::Playlist(key.clone())),
            RootlistEntry::FolderStart { id, .. } => Some(Row::Folder(id.clone())),
            RootlistEntry::FolderEnd => None,
        })
        .collect();
    for row in rows.iter().rev() {
        if position(&tree, row).is_none() {
            continue;
        }
        if neighbour(&tree, row) == neighbour(target, row) {
            continue;
        }
        let Some(place) = spotify_place(target, row, &tree) else {
            continue;
        };
        let edit = match row {
            Row::Playlist(uri) => FolderEdit::MovePlaylist {
                uri: uri.clone(),
                place,
            },
            Row::Folder(id) => FolderEdit::MoveFolder {
                id: id.clone(),
                place,
            },
        };
        make(&mut tree, edit);
    }
    for id in held.keys() {
        if !wanted.contains_key(id) {
            make(&mut tree, FolderEdit::Remove { id: id.clone() });
        }
    }
    edits
}

/// What changed from `base` to `now`, for the listener to compare: each
/// row with its new folder (`None` at the top) that moved, came or went,
/// and each folder renamed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Change {
    Moved {
        row: crate::folders::Row,
        into: Option<String>,
    },
    Added(crate::folders::Row),
    Removed(crate::folders::Row),
    Renamed {
        id: String,
        name: String,
    },
}

/// The positions in `values` of a longest rising run, not necessarily
/// side by side.
fn longest_rising(values: &[usize]) -> Vec<usize> {
    let mut tails: Vec<usize> = Vec::new();
    let mut previous: Vec<Option<usize>> = vec![None; values.len()];
    for (index, value) in values.iter().enumerate() {
        let at = tails.partition_point(|&tail| values[tail] < *value);
        previous[index] = at.checked_sub(1).map(|before| tails[before]);
        if at == tails.len() {
            tails.push(index);
        } else {
            tails[at] = index;
        }
    }
    let mut out = Vec::new();
    let mut at = tails.last().copied();
    while let Some(index) = at {
        out.push(index);
        at = previous[index];
    }
    out.reverse();
    out
}

/// The changes from `base` to `now`.
pub fn changes(base: &[RootlistEntry], now: &[RootlistEntry]) -> Vec<Change> {
    use crate::folders::Row;
    let rows = |rows: &[RootlistEntry]| -> Vec<Row> {
        rows.iter()
            .filter_map(|row| match row {
                RootlistEntry::Playlist(key) => Some(Row::Playlist(key.clone())),
                RootlistEntry::FolderStart { id, .. } => Some(Row::Folder(id.clone())),
                RootlistEntry::FolderEnd => None,
            })
            .collect()
    };
    let before = rows(base);
    let after = rows(now);
    let mut out = Vec::new();
    // In each folder, the rows that kept their order among themselves stay;
    // the fewest others moved.
    let mut kept: HashSet<Row> = HashSet::new();
    let mut by_parent: std::collections::HashMap<Option<String>, Vec<Row>> = Default::default();
    for row in &after {
        if before.contains(row)
            && crate::folders::parent(base, row) == crate::folders::parent(now, row)
        {
            by_parent
                .entry(crate::folders::parent(now, row))
                .or_default()
                .push(row.clone());
        }
    }
    for children in by_parent.values() {
        let ranks: Vec<usize> = children
            .iter()
            .map(|row| before.iter().position(|held| held == row).unwrap_or(0))
            .collect();
        for index in longest_rising(&ranks) {
            kept.insert(children[index].clone());
        }
    }
    for row in &after {
        if !before.contains(row) {
            out.push(Change::Added(row.clone()));
        } else if !kept.contains(row) {
            out.push(Change::Moved {
                row: row.clone(),
                into: crate::folders::parent(now, row),
            });
        }
    }
    for row in &before {
        if !after.contains(row) {
            out.push(Change::Removed(row.clone()));
        }
    }
    for row in now {
        if let RootlistEntry::FolderStart { id, name } = row
            && base.iter().any(|held| matches!(held, RootlistEntry::FolderStart { id: old, name: was } if old == id && was != name))
        {
            out.push(Change::Renamed {
                id: id.clone(),
                name: name.clone(),
            });
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use RootlistEntry::{FolderEnd, FolderStart, Playlist};

    fn folder(id: &str) -> RootlistEntry {
        FolderStart {
            id: id.into(),
            name: id.into(),
        }
    }

    #[test]
    fn the_first_tree_is_spotifys_with_the_pins_on_top() {
        let spotify = vec![
            folder("f"),
            Playlist("b".into()),
            Playlist("c".into()),
            FolderEnd,
            Playlist("a".into()),
        ];
        let tree = seed(&["liked".into(), "c".into()], &spotify, &[]);
        assert_eq!(
            tree,
            [
                Playlist("liked".into()),
                Playlist("c".into()),
                folder("f"),
                Playlist("b".into()),
                FolderEnd,
                Playlist("a".into()),
            ]
        );
        // A pinned folder rises whole, with a pin inside it kept once;
        // without Spotify's tree the flat order stands in.
        let tree = seed(
            &["spotify:user:u:folder:f".into(), "b".into()],
            &spotify,
            &[],
        );
        assert_eq!(&tree[..3], &spotify[..3]);
        assert_eq!(
            tree.iter()
                .filter(|row| **row == Playlist("b".into()))
                .count(),
            1,
            "{tree:?}"
        );
        assert_eq!(
            seed(&["a".into()], &[], &["b".into(), "a".into()]),
            [Playlist("a".into()), Playlist("b".into())]
        );
    }

    #[test]
    fn a_new_save_goes_just_below_liked_songs_when_it_leads() {
        let liked = crate::settings::LIKED_SONGS_KEY.to_string();
        let mut tree = vec![Playlist(liked.clone()), Playlist("a".into())];
        place(&mut tree, &["n".into()], &[]);
        assert_eq!(
            tree,
            [Playlist(liked), Playlist("n".into()), Playlist("a".into())]
        );
    }

    #[test]
    fn new_items_go_on_top_and_a_new_kind_at_the_end() {
        let mut tree = vec![Playlist("a".into())];
        assert!(place(&mut tree, &["n".into(), "a".into()], &["x".into()]));
        assert_eq!(
            tree,
            [
                Playlist("n".into()),
                Playlist("a".into()),
                Playlist("x".into())
            ]
        );
        assert!(
            !place(&mut tree, &["a".into()], &["x".into()]),
            "nothing new"
        );
    }

    fn rows(spec: &str) -> Vec<RootlistEntry> {
        // "a [f b c] d": playlists by id, a folder by its id then its rows.
        let mut out = Vec::new();
        for word in spec.split_whitespace() {
            if let Some(rest) = word.strip_prefix('[') {
                let id = rest.trim_end_matches(']');
                out.push(folder(id));
                for _ in 0..rest.len() - id.len() {
                    out.push(FolderEnd);
                }
            } else if word.ends_with(']') {
                let item = word.trim_end_matches(']');
                if !item.is_empty() {
                    out.push(Playlist(key(item)));
                }
                for _ in 0..word.len() - item.len() {
                    out.push(FolderEnd);
                }
            } else {
                out.push(Playlist(key(word)));
            }
        }
        out
    }

    fn key(item: &str) -> String {
        if item.starts_with("alb") {
            format!("spotify:album:{item}")
        } else {
            format!("spotify:playlist:{item}")
        }
    }

    #[test]
    fn spotifys_tree_comes_back_with_sirensongs_own_rows_where_they_were() {
        // An album after b, another at the top of f, and a folder of albums.
        let local = rows("alb1 [f alb2 b] [mine alb3] c");
        let spotify = rows("c [f b]");
        let merged = merge(&local, &spotify, &spotify);
        assert_eq!(merged, rows("alb1 c [f alb2 b] [mine alb3]"), "{merged:?}");
        // The new folder is for Spotify too, without the album.
        assert_eq!(projection(&merged), rows("c [f b] [mine]"));
        // Removed on Spotify: its album stays.
        let removed = merge(&local, &spotify, &rows("c [f b] [mine]"));
        assert_eq!(removed, rows("alb1 c [f alb2 b] alb3"), "{removed:?}");
        // A playlist gone from Spotify goes from here, even from a folder
        // of Sirensong's own.
        let gone = merge(&rows("x c [f b] [mine y alb3]"), &spotify, &spotify);
        assert_eq!(gone, rows("c [f b] [mine alb3]"), "{gone:?}");
    }

    #[test]
    fn a_folder_gone_from_spotify_leaves_its_own_rows_one_level_up() {
        let local = rows("[f b alb1] c");
        let merged = merge(&local, &rows("b c"), &rows("[f b] c"));
        assert_eq!(merged, rows("b alb1 c"), "{merged:?}");
    }

    #[test]
    fn the_edits_to_spotify_make_its_tree_the_target() {
        for (spotify, target) in [
            ("a b c d", "a c b d"),
            ("a b c d", "d c b a"),
            ("a [f b] c", "a c [f b]"),
            ("a [f b] c", "[f a b] c"),
            ("a b", "a [new b]"),
            ("a [f b] c", "a b c"),
            ("[f a] [g b]", "[g b [f a]]"),
        ] {
            let (spotify, target) = (rows(spotify), rows(target));
            let mut tree = spotify.clone();
            for edit in push_edits(&spotify, &target) {
                tree = crate::folders::apply(&tree, &edit).unwrap();
            }
            assert_eq!(tree, target, "{spotify:?} to {target:?}");
        }
        assert!(push_edits(&rows("a b"), &rows("a b")).is_empty());
        assert_eq!(
            push_edits(&rows("a b c d"), &rows("a c b d")).len(),
            1,
            "one move"
        );
    }

    #[test]
    fn changes_name_only_what_moved() {
        let base = rows("a b c [f d]");
        let now = rows("a c b [f d e]");
        let moved: Vec<_> = changes(&base, &now);
        // b and c swapped: one of them moved, whichever.
        let one = |id: &str| Change::Moved {
            row: crate::folders::Row::Playlist(key(id)),
            into: None,
        };
        assert_eq!(moved.len(), 2, "{moved:?}");
        assert!(
            moved.contains(&one("b")) || moved.contains(&one("c")),
            "{moved:?}"
        );
        assert!(moved.contains(&Change::Added(crate::folders::Row::Playlist(key("e")))));
        let renamed = rows("a b c [f d]")
            .into_iter()
            .map(|row| match row {
                FolderStart { id, .. } => FolderStart {
                    id,
                    name: "Gym".into(),
                },
                row => row,
            })
            .collect::<Vec<_>>();
        assert_eq!(
            changes(&base, &renamed),
            [Change::Renamed {
                id: "f".into(),
                name: "Gym".into()
            }]
        );
    }
}

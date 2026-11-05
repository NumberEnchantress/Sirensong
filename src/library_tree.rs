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
}

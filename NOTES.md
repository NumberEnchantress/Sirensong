# Notes

Decisions for this fork's own features, with dates, so later work follows
them. The product and contribution rules stay in `CONTRIBUTING.md` and
`AGENTS.md`.

## Vacuum (Unfamiliar Sounds in the app)

The Unfamiliar Sounds Firefox extension, brought into the app as Vacuum.
The reference implementation is the extension itself; its handoff notes
record its own decisions and three rounds of adversarial grading.

### 29 October 2026

- **Name:** the feature is called Vacuum. The fixed strings inside the
  derivation (`unfamiliar sounds seed:`, `picture:`) keep the extension's
  name, or stand-ins would stop matching.
- **Parity:** feature parity with the extension is the goal. Browser-only
  parts have no counterpart: the container, closing tabs when switching
  off, and asking how to open links. The toolbar button and Alt+Shift+D
  become the account menu's Vacuum item and the same shortcut. A top-bar
  button was tried and left out: with it, the search field fell below its
  readable width in the narrowest window.
- **Derivation:** ported exactly from `core/derive.js` and `core/museums.js`.
  `src/vacuum/golden-vectors.json` holds the extension's own outputs for two
  test seeds; every value is checked by a test. Names are keyed by the name
  shown, pictures by Spotify ID. Songs, podcasts, episodes and audiobooks
  are named in the album name space, as the extension does.
- **Shared seed:** committed, lightly scrambled (`src/vacuum/shared_seed.rs`),
  as decided on 29 October 2026. It is never shown, logged, put in settings
  or docs, or printed by a test. It ships in the app, as it does in the
  extension.
- **Design:** names and pictures are replaced where they are drawn (the
  brief's design b). The app keeps its real data, so search, scoring a game
  and switching off need nothing re-fetched. Views call `shown_*` helpers
  (`src/vacuum/shown.rs`) or the gate (`src/vacuum/gate.rs`); logic keeps
  the real fields. The gate fails closed: a name it cannot derive is shown
  as `???`.
- **Levels:** the gate has two levels besides off: Disguised (Vacuum) and
  Blanked (`???` and black squares), for the guessing game.
- **Years:** release years and dates stay real, as in the extension.
- **Lyrics:** shown with the names Vacuum knows swapped.
- **Approved by the user:** the museum requests (The Met, the Art Institute
  of Chicago, the Cleveland Museum of Art and their image hosts) and the
  `unicode-normalization` crate. `regex` and `resvg` were already built for
  other crates.
- **Leak scan:** `demo::tests::vacuum_shows_no_real_name_or_picture_anywhere`
  walks every page and panel of the demo with Vacuum on and everything of
  your own replaced too, and fails on any real name drawn, given to the
  screen reader, put in the window title, the media controls or the command
  line, or any real picture asked for.

### Guessing game, 29 October 2026

- **Decisions (the user):** a song without synced lyrics is guessed until a
  fifth of the way in; a game plays from a source picked per game; the
  stats show after 10 songs; points are independent per category (title
  1, artist 2, album 3, year 4, version 5).
- **Mine, from the briefs:** Name it defaults to Blanked; only typed rounds
  count in the stats; a song joined late (more than 5 seconds in) gets 30
  seconds; a miss is the title missed, or the wrong cover or colour; it
  comes back after a day, then, got right each time, after 3 and 7 days, and
  plays first. A miss when due starts it over; a miss before it is due
  changes nothing. A round skipped or ended before its reveal is not kept,
  nor one that cannot be scored (no cover read, too few covers). Numbers in
  typed answers must be exact; a version's words may come in any order. The version is read from Spotify's title, never
  guessed. The cover grid offers covers of other albums in the game, and the
  main colour is the largest of twelve colour bins over a small copy of the
  cover, a close second counting too.
- **Hiding:** the game puts Vacuum's gate at its level while a song is
  guessed, and while a song of the game not yet guessed plays before its
  round begins (so Next shows nothing real for a frame), Vacuum on or off,
  and lets it go at the reveal. Blanked hides every name in the app, not
  only the playing song's. The queue is hidden for the whole game unless the
  game shows real names. With Vacuum off, a blanked game's gate is light: it
  learns and reads nothing, and the search box keeps what was typed. A song
  outside the game, or one guessed already (Previous), is not hidden or
  guessed. Shuffle is turned off for the game and given back at its end.
- **Lyrics:** the game asks for each song's lyrics, panel open or not, to
  time the deadline; how-it-connects says so.
- **Kept:** `game.json` in the state folder; the game's setup is kept there,
  not as a setting.

### Sirensong, 29 October 2026

- **Decision (the user):** a full rename to Sirensong: the binary, the data
  folders (migrated), the installer and all text.
- **The binary** is `sirensong` (`sirensong.exe`); the package and library
  keep the name `spotifast`, so the code's paths and history stay readable.
- **Folders:** `paolino/sirensong` on every platform, and `sirensong-data`
  for a portable copy. The first start of an installed copy brings over the
  old config and state: the old state folder records its credential profile
  first, so the copy keeps the sign-in. Both folders are copied in full beside
  their places before anything moves in, and `migrated-from-spotifast` is
  written in the new state last; `migrating-from-spotifast`, written before
  the copy starts, makes every later start try again, replacing what an
  unfinished attempt left. A first start with no Spotifast folders, or with
  Sirensong's own data and no attempt under way, writes the marker and
  brings nothing over, so Sirensong's data is never replaced. Then the cache moves, the
  window's place is copied, the Windows start at login moves from the name
  Spotifast to Sirensong (on Linux, outside Flatpak, it is pointed at the
  new program), and a Connect name left at Spotifast's default becomes
  Sirensong. A portable `spotifast-data` folder is not brought over.
- **Updates:** Sirensong never installs upstream Spotifast over itself. The
  fork's installers carry `sirensong-installer.txt`, which the updater
  (still configured for upstream releases) does not take for an install it
  may update, and AppImages do not update themselves. It still tells you of
  upstream releases, so its three update messages name Spotifast.
- **Versions:** shown as `Sirensong mod 14 · Spotifast 0.14.1`: the fork's
  own version, then the upstream release it is built on.
- **Kept as Spotifast:** the credential store's service name (renaming it
  would sign everyone out); the Linux desktop entry, icon, Flatpak id and
  MPRIS bus name (`spotifast`), whose packaging and permissions are keyed to
  them, with only the names shown changed; the installers' registry
  identity, AppId and GUIDs (the MSI's name shown is Sirensong); release
  asset names; Omarchy's palette file and variables; links to
  spotifast.rocks and crmne/spotifast; the built-in Winamp skin's own name;
  developer docs (CONTRIBUTING, AGENTS, PACKAGING) and the download and
  packaging pages, which describe upstream.
- **Translations:** the name is not translated anywhere. Korean particles
  and Turkish suffixes follow the new name (이/을/은/과, 'un/'u/'a).

### Upstream 0.15.0, 1 November 2026

- **Decision (the user):** upstream releases come in by merging their tag
  into `main`, not by rebasing the fork onto them. This one is mod 15, built
  on Spotifast 0.15.0.
- **Names:** upstream's new text follows the Sirensong rename: the
  translation catalogs, the Soloist setup and settings, the Soloist guide,
  the names of the audio devices Soloist creates (`Sirensong Soloist`), and
  the Soloist preview build's app, zip and read-me, which ship this fork's
  binary.
  Update notices and the download and packaging pages still describe
  upstream, as before.
- **Soloist** (Spotify's own player, which upstream offers after sign-in)
  is taken as upstream ships it. Vacuum and the guessing game hide names
  where Sirensong draws them; they cannot change what Soloist itself
  reports.

### Sidebar icons, 2 November 2026

- **Decision (the user):** a crowded sidebar swaps words for small icons,
  reused from the app where it has one and drawn in Lucide's line style
  where it does not. Playlists: the queue's list with a play button.
  Albums: the record the app uses for albums elsewhere. Artists: Lucide's
  singer's microphone. Podcasts: the lyrics microphone. By You: the person
  from listening activity. Orders: a 1 above a 2 (Library order, drawn), a
  clock (Recently played), a clock with a plus (Recently added), the letter A
  (Name, drawn), a paintbrush (Local custom order) and a wireless signal
  (Spotify custom order).
- **When (changed the same day, the user):** the kinds, the order and By You
  show their icon and name, and the icon alone in a sidebar narrower than
  300 points. The sidebar's default width is 320 points (it was 250), so
  names show by default. Home and Search share one row as icons below 480
  points of sidebar height.
- **Map toolbar:** Zoom in has a magnifying glass. Both "only" choices say
  Only selected, with a focus frame. Size by listening is now Time
  listened, with headphones. Tree and Circles show the view icons. The
  choices show icon and name where they fit on one line, and the icon alone
  where they would wrap (the user, 2 November 2026); the "Open folder:" and
  "Filters:" labels keep their words, which tell the two pairs apart.
- **Views:** list, grid, the map as circles and the map as a tree, side by
  side in the Library's header, one click each, with icons drawn from the
  user's sketches. They take the Library heading's room in a narrow sidebar;
  the heading shows in the default width. Below the room for all four the
  single Show as button opens them. The sidebar's map drops its own Tree and
  Circles chips; Home's map keeps them.

### Upstream 0.17.1, 4 November 2026

- **Merged** upstream's 0.16.0, 0.17.0 and 0.17.1 into mod 19: Windows
  Media Player skins with sandboxed scripts, the Spotifast Tall and Wide
  skins (which keep their names, as the built-in Winamp skin does), MilkDrop
  in skins and behind lyrics, Soloist's new setup and recovery, Connect and
  playlist fixes.
- **By You keeping folders:** upstream added its own version (#1143). The
  fork's, which already did this with folder counts and the map, is kept.
- **Vacuum** also covers what 0.16 and 0.17 added: a Windows Media Player
  skin's scripts are given the song and queue as Vacuum shows them, and its
  album art area the stand-in picture; an artist's monthly listeners hide
  like followers; an upcoming release's card and an upcoming album's play
  counts follow the album rules; Copy details copies what is shown; a table
  sorted by plays falls back to its own order while counts are hidden; and a
  playlist's description keeps its links only while Vacuum is off. The leak
  test checks the skin's view and an upcoming release.
- **Names:** upstream's new "Register Spotifast" wording and the Soloist
  messages say Sirensong. The catalogs were merged entry by entry from the
  base, fork and upstream versions, not from git's text hunks, which split
  entries.

### Pins and custom orders, 4 November 2026

- **Decision (the user):** one custom order the user arranges freely, which
  can hold anything at any depth, with pins only in Spotify's order, where
  Spotify's rules and its limit of four apply. An option, on by default,
  keeps the custom order in step with Spotify's, and another chooses what
  happens when both changed (Spotify wins, local wins, or ask, the default,
  showing the conflict). Built in rounds.
- **Round 1 (mod 20):** a pin read Spotify refuses (HTTP 403 for the shared
  sign-in) keeps the pins it last reported, instead of falling back to the
  local ones, which emptied the pinned group. Pins show only in Spotify
  custom order and, until the next round replaces it, Local custom order:
  Name, Recently played and Recently added sort pinned items with the rest,
  and offer no Pin to top.
- **Round 2 (mod 21):** Local custom order became **Sirensong custom
  order**, one tree per account in `settings.json` (`library_trees`,
  `crate::library_tree`) in the rootlist's own rows, so `crate::folders` edits and checks it
  unchanged. It holds any kind: playlists, albums, artists, podcasts, Liked
  Songs and Local Files (`spotifast:local-files`). It starts from Spotify's
  tree with Liked Songs and the pins on top, waiting for Spotify's tree
  unless an edit cannot wait; a tree made flat gives way to Spotify's
  folders until the listener changes it. A new save goes just below a
  leading Liked Songs, a kind seen first goes at the end, and an empty
  folder stays in sight. The sidebar edits
  whichever tree it shows (`App::shown_tree`): folder dialogs carry `local`,
  and `Action::EditLibraryTree` changes this computer only until round 3
  syncs it. A drag in a sorted view moves the row in this order and switches
  to it. Pins show only in Spotify's order. The flat `sidebar_order` is kept
  in settings, read only to start a tree when Spotify's is unknown. The map
  still follows Spotify's tree (round 4).
- **Round 3 (mod 22):** **Keep in step with Spotify** (`library_sync`, on
  by default) makes the tree's playlists and folders Spotify's too. Each
  tree keeps `base`, Spotify's tree when the two last agreed; comparing it
  with Spotify's tree and with the tree's projection (its folders and
  playlists, `library_tree::projection`) says which side changed. Only
  Spotify: `merge` puts Spotify's tree in and Sirensong's own rows back
  after the shared row they followed (a folder Spotify removed gives its
  own rows to its parent; one new here stays whole; a playlist Spotify no
  longer holds goes). Only here: `push_edits` sends folder edits row by
  row, skipping rows whose neighbours already match. Both: `library_conflicts` decides, and
  `ask` (the default) opens a dialog listing what changed on each side
  (`library_tree::changes`, the moved set from a longest rising run).
  An edit made with the setting on sends `push_edits` at once, but only
  while Spotify is unchanged since the two agreed, so an edit never answers
  a pending conflict. A tree from before Round 3 has no `base`: unedited, it
  follows Spotify's; edited, it counts as both changed. The conflict dialog
  opens only when no other dialog is, and sign-out closes it. Kept in step,
  Spotify's tree places its playlists, not the Library's save order. With it on,
  the first tree leaves Spotify's pins where Spotify puts them. A folder
  made here goes to Spotify empty of what only Sirensong arranges.
- **Round 4 (mod 23):** the map follows Sirensong custom order while the
  Library shows it: `sidebar::custom_map_rows` walks the custom tree whole,
  every folder open, any kind in place, unplaced rows last, with no kind
  branches. A drag on it can move any row (`row_of(node, custom)`), is
  checked against `App::shown_tree`, and goes through `tree_action`, so it
  edits the custom order and syncs as any other edit.

### Still open

- Soloist, not verified here: whether Spotify's Soloist player shows the
  playing song to the system's media controls while Vacuum or the game
  hides it. Until checked, use the built-in player with Vacuum or the game.

- Sirensong, not verified here: the Linux packaging tests, the MSI and Inno
  Setup builds, and macOS. A Mac build of the fork would still take
  upstream updates (the updater matches its bundle), and the Homebrew cask's
  clean-up paths still list only Spotifast's folders.

- Guessing game: with Vacuum off and the game showing stand-ins, the gate is
  built at each round and let go at each reveal, learning the library again
  each time; a blanked game, and any game with Vacuum on, do not. An Honor
  round revealed but not moved past is lost if the app quits, and shuffle
  stays off if the app quits mid-game. A song librespot skips as unplayable
  stays to be played, so Continue the game offers it again.

- Playlist verdicts are not kept between runs; a playlist judged about a
  person from its songs is judged again from its title next time.
- Spotify's own format for its playlists (artist mixes, charts) is not read,
  so rule 0 of the extension's judgement does not apply; the title rules
  catch the same mixes by their titles.
- Save as playlist on a radio, and Save queue as playlist, name the new
  playlist after the real station or date. It is then your own playlist,
  which keeps its title unless Replace my own playlists is on.
- Edit details on your own playlist shows its real name, description and
  cover, even with Replace my own playlists on, as editing needs them.
- The app shows no artist biographies, so Hide artist descriptions has
  nothing to hide yet; it is kept for parity and for when it does.
- Accent colours from covers are off while Vacuum is on: reading them from
  a real cover would tell of it.
- A grader's report (29 October 2026) found leaks the first leak scan
  missed; all fixed, and the scan now also visits Settings, a wide window
  (Added by), the Add to playlist list and a search by stand-in.

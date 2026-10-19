# opendj-virtualdj-import

A private add-on for openDJ that brings a VirtualDJ library into the
collection at start. It reads `database.xml` from
`~/Library/Application Support/VirtualDJ`, `~/Documents/VirtualDJ` or
`~/.vdj` (or the file `OPENDJ_VDJ_DATABASE` names), and the playlists,
folders and history next to it, only when something has changed since last
time. It never writes to any of them.

Each local track keeps from VirtualDJ:
- title (with the remix), artist, album and genre
- year, track number, record label, composer and remixer
- stars, colour, comment, play count, date added and last played
- BPM, beat grid and key, used as they are without re-analysis
- hot cues with their names and colours, and saved loops (on the pads after
  the hot cues)
- named sections (remix points), automix points and the track's gain

What VirtualDJ leaves empty is filled in from the file's own tags (files
still in iCloud are left alone, so nothing is downloaded).

The Grouping and the User1/User2 hashtags show under *Groupings and Tags* in
openDJ's tree, and search covers them. Grids and cues you edit in openDJ are
kept on later imports. Streaming entries and VirtualDJ's offline copies of
streamed tracks (`.vdjcache`) are skipped.

## Playlists, folders and history

*Settings > Library > From VirtualDJ* picks what comes across, into a
*VirtualDJ* folder in openDJ's tree:

| Switch | What | Default |
|---|---|---|
| Playlists | `Playlists/*.m3u`, in their folders and order | on |
| Virtual folders | `Folders/**/*.vdjfolder` that list songs | on |
| Filter folders | `.vdjfolder` filters, as smart lists where openDJ's search can say the same thing (`genre is house and bpm > 120`); others are skipped | off |
| History | `History/*.m3u`, one session per day, in the History tab | on |

The lists are made again whenever they change in VirtualDJ, so change them
there. Streamed tracks in them are left out, as openDJ only plays files.

## Building it into openDJ

In a checkout of openDJ, with this repository cloned next to it:

1. In `crates/app/Cargo.toml`:

   ```toml
   [dependencies]
   opendj-addon-vdj = { path = "../../../opendj-virtualdj-import", optional = true }

   [features]
   vdj = ["dep:opendj-addon-vdj"]
   ```

   Also point this repo's `mixxx-engine` and `mixxx-plugins` dependencies at
   the same checkout (`path = "../opendj/crates/engine"` and
   `path = "../opendj/crates/plugins"`), so both use one copy.
2. In `crates/app/src/stores.rs`, register it in `importers()`:

   ```rust
   pub fn importers() -> Vec<Box<dyn LibraryImport>> {
       #[allow(unused_mut)]
       let mut all: Vec<Box<dyn LibraryImport>> = Vec::new();
       #[cfg(feature = "vdj")]
       all.push(Box::new(opendj_addon_vdj::VirtualDj));
       all
   }
   ```
3. `cargo run --release --features vdj`

Tracks imported before the add-on was split out keep their data; the first
start with it reads the database once more.

Set `OPENDJ_VDJ_TEST_DB` to a real `database.xml` to run the extra test
against it (`cargo test`).

## Licence

GPL-2.0-or-later, the same as openDJ. See [LICENSE](LICENSE).

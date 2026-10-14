# opendj-virtualdj-import

A private add-on for openDJ that brings a VirtualDJ library into the
collection at start. It reads `database.xml` from
`~/Library/Application Support/VirtualDJ`, `~/Documents/VirtualDJ` or
`~/.vdj` (or the file `OPENDJ_VDJ_DATABASE` names), only when it has changed
since last time, and never writes to it.

Each local track keeps from VirtualDJ:
- title (with the remix), artist, album and genre
- stars, colour, comment and play count
- BPM, beat grid and key, used as they are without re-analysis
- hot cues 1 to 8

The Grouping and the User1/User2 hashtags show under *Groupings and Tags* in
openDJ's tree, and search covers them. Grids and cues you edit in openDJ are
kept on later imports. Streaming entries and VirtualDJ's offline copies of
streamed tracks (`.vdjcache`) are skipped.

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

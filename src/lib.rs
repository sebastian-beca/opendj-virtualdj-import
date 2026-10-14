//! An openDJ add-on that brings a VirtualDJ library into the collection at
//! start (see [`vdj`]). It isn't part of openDJ: see the README for building
//! it into a copy of the app for your own use.

use std::path::{Path, PathBuf};

use mixxx_engine::Track;
use mixxx_plugins::LibraryImport;

pub mod vdj;

/// VirtualDJ's `database.xml`, read whenever it has changed since last time.
pub struct VirtualDj;

impl LibraryImport for VirtualDj {
    fn id(&self) -> &str {
        "vdj"
    }

    fn database(&self) -> Option<PathBuf> {
        vdj::default_database()
    }

    fn read(&self, database: &Path) -> Result<Vec<Track>, String> {
        let import = vdj::import(database, 0, Path::exists)?;
        eprintln!(
            "VirtualDJ library: {} tracks; skipped {} streaming, {} cached, {} missing",
            import.tracks.len(),
            import.streaming,
            import.cached,
            import.missing
        );
        Ok(import.tracks.iter().map(|t| (**t).clone()).collect())
    }
}

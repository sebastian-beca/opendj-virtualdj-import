//! Reads VirtualDJ's library, `database.xml`, so a collection built in
//! VirtualDJ comes across with its tags, ratings, colours, beat grids, keys
//! and hot cues. The file is only read, never written, as VirtualDJ asks of
//! other apps (<https://virtualdj.com/wiki/VDJ_database.html>).
//!
//! Each `<Song FilePath=…>` holds `<Tags>` (Author, Title, Remix, Album,
//! Genre, Stars, Grouping, User1, User2…), `<Infos>` (SongLength,
//! PlayCount, UserColor…), `<Scan>` (Bpm as seconds per beat, Key, Phase)
//! and `<Poi>` points: `beatgrid` anchors, numbered `cue`s, saved `loop`s,
//! `remix` sections and `automix` points. Streaming entries
//! (`netsearch://…`) and VirtualDJ's protected offline copies
//! (`.vdjcache`) are counted but not imported: they can't be played here.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use mixxx_engine::beats::{BeatGrid, GridMarker};
use mixxx_engine::{Hotcue, LibraryInfo, Source, Track};

/// Where VirtualDJ keeps its main database: `OPENDJ_VDJ_DATABASE`, else
/// the first of VirtualDJ's home folders that has one.
pub fn default_database() -> Option<PathBuf> {
    if let Some(p) = std::env::var_os("OPENDJ_VDJ_DATABASE") {
        return Some(PathBuf::from(p));
    }
    let home = PathBuf::from(std::env::var_os("HOME")?);
    [
        "Library/Application Support/VirtualDJ/database.xml",
        "Documents/VirtualDJ/database.xml",
        ".vdj/database.xml",
    ]
    .iter()
    .map(|p| home.join(p))
    .find(|p| p.is_file())
}

/// What an import found.
#[derive(Debug, Default)]
pub struct Import {
    pub tracks: Vec<Arc<Track>>,
    /// Streaming entries (TIDAL, SoundCloud, Deezer…), not imported.
    pub streaming: usize,
    /// VirtualDJ's offline copies of streamed tracks, not imported.
    pub cached: usize,
    /// Local files that are no longer where the database says.
    pub missing: usize,
}

/// Reads the database at `path`. Tracks get ids from `first_id`; `exists`
/// says whether a file is there (so tests need no real files).
pub fn import(
    path: &Path,
    first_id: usize,
    exists: impl Fn(&Path) -> bool,
) -> Result<Import, String> {
    let text = std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
    parse(&text, first_id, exists)
}

pub fn parse(
    text: &str,
    first_id: usize,
    exists: impl Fn(&Path) -> bool,
) -> Result<Import, String> {
    let doc = roxmltree::Document::parse(text).map_err(|e| e.to_string())?;
    let mut out = Import::default();
    for song in doc
        .root_element()
        .children()
        .filter(|n| n.has_tag_name("Song"))
    {
        let Some(file) = song.attribute("FilePath") else {
            continue;
        };
        if file.contains("://") {
            out.streaming += 1;
            continue;
        }
        let lower = file.to_ascii_lowercase();
        if lower.ends_with(".vdjcache") || lower.ends_with(".vdjedit") {
            out.cached += 1;
            continue;
        }
        let path = PathBuf::from(file);
        if !exists(&path) {
            out.missing += 1;
            continue;
        }
        let id = first_id + out.tracks.len();
        out.tracks.push(Arc::new(song_track(id, path, song)));
    }
    Ok(out)
}

fn song_track(id: usize, path: PathBuf, song: roxmltree::Node) -> Track {
    let child = |name: &str| song.children().find(|n| n.has_tag_name(name));
    let attr = |node: Option<roxmltree::Node>, name: &str| -> String {
        node.and_then(|n| n.attribute(name))
            .unwrap_or_default()
            .trim()
            .to_string()
    };
    let number = |node: Option<roxmltree::Node>, name: &str| -> Option<f64> {
        node.and_then(|n| n.attribute(name))
            .and_then(|v| v.trim().parse::<f64>().ok())
            .filter(|v| v.is_finite())
    };
    let tags = child("Tags");
    let infos = child("Infos");
    let scan = child("Scan");

    let mut title = attr(tags, "Title");
    let remix = attr(tags, "Remix");
    if !remix.is_empty() && !title.to_lowercase().contains(&remix.to_lowercase()) {
        title = format!("{title} ({remix})");
    }
    if title.is_empty() {
        title = path
            .file_stem()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_default();
    }

    // Scan's Bpm is seconds per beat; a Bpm typed into the tags is too.
    let seconds_per_beat = number(scan, "Bpm").or_else(|| number(tags, "Bpm"));
    let points: Vec<roxmltree::Node> = song.children().filter(|n| n.has_tag_name("Poi")).collect();
    let grid_points: Vec<&roxmltree::Node> = points
        .iter()
        .filter(|p| p.attribute("Type") == Some("beatgrid"))
        .collect();
    let grid = grid_points.first().copied();
    // The first grid point anchors the beats; its own Bpm (in beats per
    // minute) wins where the tempo was set by hand.
    let bpm = grid
        .and_then(|g| number(Some(*g), "Bpm"))
        .or_else(|| seconds_per_beat.filter(|s| *s > 0.0).map(|s| 60.0 / s))
        .filter(|b| (20.0..=400.0).contains(b))
        .unwrap_or(0.0);
    let first_beat = grid
        .and_then(|g| number(Some(*g), "Pos"))
        .or_else(|| number(scan, "Phase"))
        .unwrap_or(0.0)
        .max(0.0);
    let key = Some(attr(scan, "Key"))
        .filter(|k| !k.is_empty())
        .unwrap_or_else(|| attr(tags, "Key"));

    let hotcues = points
        .iter()
        .filter(|p| p.attribute("Type") == Some("cue"))
        .filter_map(|p| {
            let n = number(Some(*p), "Num")? as i64;
            let seconds = number(Some(*p), "Pos")?;
            (n >= 1).then(|| Hotcue {
                number: n as usize - 1,
                seconds,
                name: p.attribute("Name").unwrap_or_default().to_string(),
            })
        })
        .collect();
    let labels = ["User1", "User2"]
        .iter()
        .flat_map(|k| {
            attr(tags, k)
                .split_whitespace()
                .map(str::to_string)
                .collect::<Vec<_>>()
        })
        .collect();
    let color = number(infos, "UserColor").map(|c| (c as u64 & 0xff_ffff) as u32);

    // Each further grid point starts a section at its own tempo (VirtualDJ
    // writes them for tracks whose tempo drifts).
    let mut markers = Vec::new();
    let mut tempo = bpm;
    for p in &grid_points {
        let Some(pos) = number(Some(**p), "Pos") else {
            continue;
        };
        tempo = number(Some(**p), "Bpm")
            .filter(|b| (20.0..=400.0).contains(b))
            .unwrap_or(tempo);
        markers.push(GridMarker {
            seconds: pos,
            bpm: tempo,
        });
    }
    let mut track = Track {
        id,
        artist: attr(tags, "Author"),
        title,
        album: attr(tags, "Album"),
        genre: attr(tags, "Genre"),
        rating: number(tags, "Stars").unwrap_or(0.0).clamp(0.0, 5.0) as u8,
        bpm,
        key,
        duration: number(infos, "SongLength").unwrap_or(0.0),
        first_beat,
        source: Source::File(path),
        waveform: Arc::default(),
        audio: None,
        cover: None,
        grid: Default::default(),
        info: LibraryInfo {
            comment: song
                .children()
                .find(|n| n.has_tag_name("Comment"))
                .and_then(|n| n.text())
                .unwrap_or_default()
                .trim()
                .to_string(),
            grouping: attr(tags, "Grouping"),
            labels,
            color,
            play_count: number(infos, "PlayCount").unwrap_or(0.0) as u32,
            hotcues,
            fixed_grid: bpm > 0.0,
            ..Default::default()
        },
    };
    if markers.len() > 1 && bpm > 0.0 {
        track.set_grid(BeatGrid::new(markers));
    }
    track
}

#[cfg(test)]
mod tests {
    use super::*;

    const DB: &str = r##"<?xml version="1.0" encoding="UTF-8"?>
<VirtualDJ_Database Version="2026">
 <Song FilePath="/music/a.flac" FileSize="1">
  <Tags Author="Artist A" Title="Song A" Remix="Club Mix" Genre="House" Album="LP" Stars="4" Grouping="11 YOGA" User1="#uplifting" User2="#synth #vox" Flag="1" />
  <Infos SongLength="300.5" PlayCount="7" UserColor="4294959104" />
  <Comment>nice one</Comment>
  <Scan Version="801" Bpm="0.468750" Phase="0.25" Key="Am" Flag="32768" />
  <Poi Pos="0.30" Type="beatgrid" />
  <Poi Pos="37.5" Num="1" Type="cue" Name="Drop" />
  <Poi Pos="71.9" Num="3" Type="cue" />
  <Poi Name="Break 1" Pos="100" Type="remix" />
  <Poi Pos="10" Type="automix" Point="fadeStart" />
 </Song>
 <Song FilePath="/music/b.mp3" FileSize="1">
  <Tags Author="B" Title="Song B" Bpm="0.5" />
  <Infos SongLength="200" />
  <Poi Pos="1.0" Type="beatgrid" Bpm="73.7" />
  <Poi Pos="40.0" Type="beatgrid" Bpm="80.0" Phrase="1" />
 </Song>
 <Song FilePath="netsearch://td123" FileSize="1"><Tags Title="Streamed" /></Song>
 <Song FilePath="/Users/x/Library/Application Support/VirtualDJ/Cache/c.vdjcache" FileSize="1"><Tags Title="Cached" /></Song>
 <Song FilePath="/music/gone.mp3" FileSize="1"><Tags Title="Gone" /></Song>
</VirtualDJ_Database>"##;

    #[test]
    fn imports_local_tracks_with_their_grid_cues_and_tags() {
        let out = parse(DB, 100, |p| !p.ends_with("gone.mp3")).unwrap();
        assert_eq!(
            (out.tracks.len(), out.streaming, out.cached, out.missing),
            (2, 1, 1, 1)
        );
        let a = &out.tracks[0];
        assert_eq!(a.id, 100);
        assert_eq!(a.title, "Song A (Club Mix)");
        assert_eq!(
            (a.artist.as_str(), a.genre.as_str(), a.rating),
            ("Artist A", "House", 4)
        );
        assert!((a.bpm - 128.0).abs() < 1e-9);
        assert_eq!(a.first_beat, 0.30);
        assert_eq!(a.key, "Am");
        assert_eq!(a.duration, 300.5);
        assert!(a.info.fixed_grid);
        assert_eq!(a.info.grouping, "11 YOGA");
        assert_eq!(a.info.labels, ["#uplifting", "#synth", "#vox"]);
        assert_eq!(a.info.color, Some(0xffe000));
        assert_eq!(a.info.play_count, 7);
        assert_eq!(a.info.comment, "nice one");
        assert_eq!(
            a.info.hotcues,
            [
                Hotcue {
                    number: 0,
                    seconds: 37.5,
                    name: "Drop".into()
                },
                Hotcue {
                    number: 2,
                    seconds: 71.9,
                    name: String::new()
                },
            ]
        );
        // A grid point's own Bpm wins over the tag's, and further points
        // start sections at their own tempo.
        let b = &out.tracks[1];
        assert!(b.grid.is_dynamic());
        assert!((b.bpm_at(10.0) - 73.7).abs() < 0.2);
        assert!((b.bpm_at(50.0) - 80.0).abs() < 1e-9);
        assert!((b.bpm - 80.0).abs() < 1e-9, "the longer section's tempo");
        assert!((b.beat_at(40.0) - b.beat_at(40.0).round()).abs() < 1e-9);
        assert!(a.grid.is_empty(), "one grid point is a steady tempo");
    }

    #[test]
    fn reads_a_real_database_when_given_one() {
        // Set OPENDJ_VDJ_TEST_DB to check against a real VirtualDJ library.
        let Some(path) = std::env::var_os("OPENDJ_VDJ_TEST_DB") else {
            return;
        };
        let out = import(Path::new(&path), 0, |_| true).unwrap();
        let cues: usize = out.tracks.iter().map(|t| t.info.hotcues.len()).sum();
        let gridded = out.tracks.iter().filter(|t| t.bpm > 0.0).count();
        eprintln!(
            "{} tracks ({} with a grid, {} hot cues), {} streaming, {} cached, {} missing",
            out.tracks.len(),
            gridded,
            cues,
            out.streaming,
            out.cached,
            out.missing
        );
        assert!(!out.tracks.is_empty());
    }
}

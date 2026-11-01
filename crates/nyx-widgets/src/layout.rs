//! Tile sizes and pages: a 4-column grid, filled first-fit in the order the user gave.
//!
//! Saved as TOML (`~/.config/nyx/control-center.toml`):
//!
//! ```toml
//! [[pages]]
//! tiles = [
//!   { widget = "wifi", size = "2x1" },
//!   { widget = "volume", size = "4x1" },
//!   { widget = "home", size = "2x2", options = { room = "Living room" } },
//! ]
//! ```

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

pub const COLUMNS: u32 = 4;

/// A tile's size in grid cells, written `"2x1"` (columns × rows).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Size {
    pub cols: u32,
    pub rows: u32,
}

impl Size {
    pub const S1X1: Size = Size { cols: 1, rows: 1 };
    pub const S2X1: Size = Size { cols: 2, rows: 1 };
    pub const S2X2: Size = Size { cols: 2, rows: 2 };
    pub const S4X1: Size = Size { cols: 4, rows: 1 };
    pub const S4X2: Size = Size { cols: 4, rows: 2 };

    pub fn parse(s: &str) -> Option<Size> {
        let (c, r) = s.split_once('x')?;
        let size = Size { cols: c.trim().parse().ok()?, rows: r.trim().parse().ok()? };
        (size.cols >= 1 && size.cols <= COLUMNS && size.rows >= 1 && size.rows <= 4).then_some(size)
    }
}

impl std::fmt::Display for Size {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}x{}", self.cols, self.rows)
    }
}

impl Serialize for Size {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&self.to_string())
    }
}

impl<'de> Deserialize<'de> for Size {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Size, D::Error> {
        let s = String::deserialize(d)?;
        Size::parse(&s).ok_or_else(|| serde::de::Error::custom(format!("not a tile size: {s} (try 1x1, 2x1, 2x2, 4x1)")))
    }
}

/// One tile on a page: which widget, how big, and its own settings.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Tile {
    pub widget: String,
    pub size: Size,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub options: BTreeMap<String, String>,
}

impl Tile {
    pub fn new(widget: &str, size: Size) -> Tile {
        Tile { widget: widget.into(), size, options: BTreeMap::new() }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Page {
    #[serde(default)]
    pub tiles: Vec<Tile>,
}

/// Where a tile landed: column and row of its top-left cell.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Placed {
    pub col: u32,
    pub row: u32,
    pub size: Size,
}

/// Place tiles in order, each in the first spot (row by row, left to right) where it
/// fits. Wider than the grid is narrowed to fit.
pub fn pack(sizes: &[Size]) -> Vec<Placed> {
    let mut taken: Vec<[bool; COLUMNS as usize]> = Vec::new();
    let mut out = Vec::with_capacity(sizes.len());
    for &size in sizes {
        let size = Size { cols: size.cols.min(COLUMNS), rows: size.rows };
        let mut row = 0u32;
        let spot = loop {
            while taken.len() < (row + size.rows) as usize {
                taken.push([false; COLUMNS as usize]);
            }
            let free = |c: u32| (row..row + size.rows).all(|r| (c..c + size.cols).all(|cc| !taken[r as usize][cc as usize]));
            if let Some(col) = (0..=COLUMNS - size.cols).find(|&c| free(c)) {
                break (col, row);
            }
            row += 1;
        };
        for r in spot.1..spot.1 + size.rows {
            for c in spot.0..spot.0 + size.cols {
                taken[r as usize][c as usize] = true;
            }
        }
        out.push(Placed { col: spot.0, row: spot.1, size });
    }
    out
}

/// Rows a packed page uses.
pub fn rows(placed: &[Placed]) -> u32 {
    placed.iter().map(|p| p.row + p.size.rows).max().unwrap_or(0)
}

/// Move a tile within a page or to another page (at `to_index` there).
pub fn move_tile(pages: &mut [Page], from: (usize, usize), to_page: usize, to_index: usize) -> bool {
    let (fp, fi) = from;
    if fp >= pages.len() || fi >= pages[fp].tiles.len() || to_page >= pages.len() {
        return false;
    }
    let tile = pages[fp].tiles.remove(fi);
    let at = to_index.min(pages[to_page].tiles.len());
    pages[to_page].tiles.insert(at, tile);
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sizes_parse_and_print() {
        assert_eq!(Size::parse("2x1"), Some(Size::S2X1));
        assert_eq!(Size::parse(" 4 x 1 "), Some(Size::S4X1));
        assert_eq!(Size::parse("5x1"), None);
        assert_eq!(Size::parse("0x1"), None);
        assert_eq!(Size::S2X2.to_string(), "2x2");
    }

    #[test]
    fn packs_first_fit() {
        use Size as S;
        // The mockup's first page: two 2×1, a 2×1 and two 1×1, a 4×1, a 2×2 beside
        // two stacked 2×1, then four 1×1.
        let sizes =
            [S::S2X1, S::S2X1, S::S2X1, S::S1X1, S::S1X1, S::S4X1, S::S2X2, S::S2X1, S::S2X1, S::S1X1, S::S1X1, S::S1X1, S::S1X1];
        let p = pack(&sizes);
        let at: Vec<(u32, u32)> = p.iter().map(|p| (p.col, p.row)).collect();
        assert_eq!(
            at,
            vec![(0, 0), (2, 0), (0, 1), (2, 1), (3, 1), (0, 2), (0, 3), (2, 3), (2, 4), (0, 5), (1, 5), (2, 5), (3, 5)]
        );
        assert_eq!(rows(&p), 6);
    }

    #[test]
    fn small_tiles_fill_gaps() {
        // A 2×2 leaves room on its right for 1×1s before a new row starts.
        let p = pack(&[Size::S2X2, Size::S4X1, Size::S1X1, Size::S1X1, Size::S1X1]);
        let at: Vec<(u32, u32)> = p.iter().map(|p| (p.col, p.row)).collect();
        assert_eq!(at, vec![(0, 0), (0, 2), (2, 0), (3, 0), (2, 1)]);
    }

    #[test]
    fn moves_between_pages() {
        let mut pages = vec![
            Page { tiles: vec![Tile::new("a", Size::S1X1), Tile::new("b", Size::S1X1)] },
            Page { tiles: vec![Tile::new("c", Size::S1X1)] },
        ];
        assert!(move_tile(&mut pages, (0, 0), 1, 0));
        assert_eq!(pages[0].tiles.iter().map(|t| t.widget.as_str()).collect::<Vec<_>>(), vec!["b"]);
        assert_eq!(pages[1].tiles.iter().map(|t| t.widget.as_str()).collect::<Vec<_>>(), vec!["a", "c"]);
        assert!(!move_tile(&mut pages, (5, 0), 0, 0));
    }

    #[test]
    fn layout_file() {
        let text = r#"
[[pages]]
tiles = [
  { widget = "wifi", size = "2x1" },
  { widget = "home", size = "2x2", options = { room = "Living room" } },
]
"#;
        #[derive(Deserialize)]
        struct F {
            pages: Vec<Page>,
        }
        let f: F = toml::from_str(text).unwrap();
        assert_eq!(f.pages[0].tiles[1].options["room"], "Living room");
        assert!(toml::from_str::<F>("[[pages]]\ntiles = [{ widget = \"x\", size = \"9x9\" }]").is_err());
    }
}

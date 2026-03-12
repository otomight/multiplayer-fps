/// Tile values
pub const FLOOR: u8 = 0;
pub const WALL: u8 = 1;

/// A maze level. Tiles are stored row-major: tiles[y * width + x].
/// All border tiles are walls. Spawn points are guaranteed to be on floor tiles.
#[derive(Debug, Clone)]
pub struct Map {
    pub id: u8,
    pub width: usize,
    pub height: usize,
    pub tiles: Vec<u8>,
    /// Player spawn positions in tile-space (center of the tile).
    pub spawn_points: Vec<(f32, f32)>,
}

impl Map {
    /// Returns true when the tile at (x, y) blocks movement.
    pub fn wall_at(&self, x: i32, y: i32) -> bool {
        if x < 0 || y < 0 || x as usize >= self.width || y as usize >= self.height {
            return true;
        }
        self.tiles[y as usize * self.width + x as usize] != FLOOR
    }

    /// Floating-point variant used by the collision system.
    /// Uses floor() so 1.9 and 1.1 both map to tile 1, matching raycaster expectations.
    pub fn wall_at_f(&self, x: f32, y: f32) -> bool {
        self.wall_at(x.floor() as i32, y.floor() as i32)
    }
}

// ── Level builder ─────────────────────────────────────────────────────────────

/// Build a Map from a slice of equal-length strings.
/// '1' → WALL, anything else → FLOOR.
fn from_rows(id: u8, rows: &[&str], spawn_points: Vec<(f32, f32)>) -> Map {
    let height = rows.len();
    let width = rows[0].len();
    let mut tiles = Vec::with_capacity(width * height);
    for row in rows {
        for ch in row.chars() {
            tiles.push(if ch == '1' { WALL } else { FLOOR });
        }
    }
    Map { id, width, height, tiles, spawn_points }
}

// ── Level 0 — 16×16, simple (few dead ends) ──────────────────────────────────
//
//  Legend: 1=wall  0=floor   S=spawn (treated as floor)
//
//  1111111111111111
//  1S00000000000S01   row 1
//  1011101110111101
//  1010001000100101
//  1010101010101001
//  1000101010000001
//  1111101010111101
//  1000001010000001
//  1011111010111101
//  1010000010100001
//  1010111110100101
//  1010100000011101
//  1010101111010001
//  1S00101000000S01   row 13
//  1011101011111101
//  1111111111111111

#[rustfmt::skip]
const LEVEL_0: &[&str] = &[
    "1111111111111111",
    "1000000000000001",
    "1011101110111101",
    "1010001000100101",
    "1010101010101001",
    "1000101010000001",
    "1111101010111101",
    "1000001010000001",
    "1011111010111101",
    "1010000010100001",
    "1010111110100101",
    "1010100000011101",
    "1010101111010001",
    "1000101000000001",
    "1011101011111101",
    "1111111111111111",
];

// ── Level 1 — 20×20, medium (more dead ends) ─────────────────────────────────

#[rustfmt::skip]
const LEVEL_1: &[&str] = &[
    "11111111111111111111",
    "10000100001000010001",
    "10111101011011110101",
    "10100001010000010101",
    "10101111011111010101",
    "10101000010001010001",
    "10101011110001011111",
    "10001010000001000001",
    "11111010111111011101",
    "10000010100000010001",
    "10111110101111110101",
    "10100010101000000101",
    "10101110101011111101",
    "10100000101010000001",
    "10111111101010111101",
    "10000000001010100001",
    "11110101011111010001",
    "10000101010010100001",
    "10111111010010100001",
    "11111111111111111111",
];

// ── Level 2 — 24×24, hard (complex, many dead ends) ──────────────────────────

#[rustfmt::skip]
const LEVEL_2: &[&str] = &[
    "111111111111111111111111",
    "100001000010000100001001",
    "101111011010111101111101",
    "101000010010100000100001",
    "101011110110101111100001",
    "101010000000101000000001",
    "101011111111101011111101",
    "101010000000001010000001",
    "101010111111111010111101",
    "101010100000000010100001",
    "101010101111111110101111",
    "100010101000000000100001",
    "111110101011111111101101",
    "100000101010000000001001",
    "101111101010111111110101",
    "101000001010100000000101",
    "101011111110101111110001",
    "101010000000101000000101",
    "101011111111101011111101",
    "101000000000001010000001",
    "101111111111111010111111",
    "100000000000001010000001",
    "100111111111111011111101",
    "111111111111111111111111",
];

// ── Public API ────────────────────────────────────────────────────────────────

/// Return a clone of the requested level. Panics on unknown id.
pub fn level(id: u8) -> Map {
    match id {
        0 => from_rows(
            0,
            LEVEL_0,
            vec![(1.5, 1.5), (13.5, 1.5), (1.5, 13.5), (13.5, 13.5)],
        ),
        1 => from_rows(
            1,
            LEVEL_1,
            vec![(1.5, 1.5), (17.5, 1.5), (1.5, 17.5), (17.5, 17.5)],
        ),
        2 => from_rows(
            2,
            LEVEL_2,
            vec![(1.5, 1.5), (21.5, 1.5), (1.5, 21.5), (21.5, 21.5)],
        ),
        _ => panic!("unknown level id: {id}"),
    }
}

pub const LEVEL_COUNT: u8 = 3;

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn check_level(id: u8) {
        let map = level(id);

        // All rows must have the same width
        assert_eq!(map.tiles.len(), map.width * map.height);

        // Borders must all be walls
        for x in 0..map.width {
            assert_eq!(map.tiles[x], WALL, "level {id}: top border at x={x}");
            assert_eq!(
                map.tiles[(map.height - 1) * map.width + x],
                WALL,
                "level {id}: bottom border at x={x}"
            );
        }
        for y in 0..map.height {
            assert_eq!(map.tiles[y * map.width], WALL, "level {id}: left border at y={y}");
            assert_eq!(
                map.tiles[y * map.width + map.width - 1],
                WALL,
                "level {id}: right border at y={y}"
            );
        }

        // Spawn points must be on floor tiles
        for (sx, sy) in &map.spawn_points {
            assert!(
                !map.wall_at(*sx as i32, *sy as i32),
                "level {id}: spawn ({sx},{sy}) is on a wall"
            );
        }
    }

    #[test]
    fn level_0_is_valid() {
        check_level(0);
    }

    #[test]
    fn level_1_is_valid() {
        check_level(1);
    }

    #[test]
    fn level_2_is_valid() {
        check_level(2);
    }
}

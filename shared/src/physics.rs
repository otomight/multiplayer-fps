use crate::{constants::PLAYER_RADIUS, map::Map};

/// Move a player from (x, y) by (dx, dy), sliding along walls.
///
/// dx/dy must already be scaled by delta_time (i.e. `move_speed * dt`).
/// Axes are resolved independently so the player slides along walls
/// instead of getting stuck on corners.
pub fn try_move(map: &Map, x: f32, y: f32, dx: f32, dy: f32) -> (f32, f32) {
    let r = PLAYER_RADIUS;
    let mut rx = x;
    let mut ry = y;

    // ── X axis ───────────────────────────────────────────────────────────────
    if dx != 0.0 {
        let nx = x + dx;
        let lead = nx + r * dx.signum(); // leading edge in X direction
        if !map.wall_at_f(lead, y - r)
            && !map.wall_at_f(lead, y)
            && !map.wall_at_f(lead, y + r)
        {
            rx = nx;
        }
    }

    // ── Y axis (uses updated rx so corner sliding works) ─────────────────────
    if dy != 0.0 {
        let ny = y + dy;
        let lead = ny + r * dy.signum(); // leading edge in Y direction
        if !map.wall_at_f(rx - r, lead)
            && !map.wall_at_f(rx, lead)
            && !map.wall_at_f(rx + r, lead)
        {
            ry = ny;
        }
    }

    (rx, ry)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::map::level;

    #[test]
    fn move_into_open_space() {
        let map = level(0);
        // Spawn is at (1.5, 1.5) which is an open tile
        let (nx, ny) = try_move(&map, 1.5, 1.5, 0.1, 0.0);
        assert!(nx > 1.5, "should have moved right");
        assert_eq!(ny, 1.5);
    }

    #[test]
    fn blocked_by_wall() {
        let map = level(0);
        // Move far enough left to hit the border wall at x=0
        let (nx, _) = try_move(&map, 1.5, 1.5, -10.0, 0.0);
        assert!(nx > 0.0, "should not pass through the border wall");
    }
}

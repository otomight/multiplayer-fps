use shared::{map::Map, player::PlayerState};

use super::{SCREEN_H, SCREEN_W};

// ── Palette ───────────────────────────────────────────────────────────────────

const CEILING: [u8; 4] = [20, 20, 40, 255];
const FLOOR: [u8; 4] = [45, 30, 15, 255];
const WALL_X: [u8; 4] = [200, 200, 200, 255];
const WALL_Y: [u8; 4] = [110, 110, 110, 255];
const SPRITE: [u8; 4] = [220, 70, 70, 255];

// ── Public entry point ────────────────────────────────────────────────────────

pub fn render(frame: &mut [u8], player: &PlayerState, all_players: &[PlayerState], map: &Map) {
    let dir_x = player.angle.cos();
    let dir_y = player.angle.sin();
    let plane_x = -dir_y * 0.66;
    let plane_y = dir_x * 0.66;

    let mut z_buffer = [f32::INFINITY; SCREEN_W];

    // ── Phase 1: DDA per column — fills z_buffer + per-column wall metrics ────
    //
    // We separate the geometry pass from the pixel-write pass so that the write
    // pass can iterate row-major (sequential addresses) instead of column-major
    // (stride-2560 bytes), which would thrash L2/L3 cache.

    let mut col_y_start = [0usize; SCREEN_W];
    let mut col_y_end   = [0usize; SCREEN_W];
    let mut col_color   = [[0u8; 4]; SCREEN_W];

    for x in 0..SCREEN_W {
        let camera_x = 2.0 * x as f32 / SCREEN_W as f32 - 1.0;
        let ray_dx = dir_x + plane_x * camera_x;
        let ray_dy = dir_y + plane_y * camera_x;

        let mut map_x = player.x as i32;
        let mut map_y = player.y as i32;

        let ddx = if ray_dx == 0.0 { f32::INFINITY } else { (1.0 / ray_dx).abs() };
        let ddy = if ray_dy == 0.0 { f32::INFINITY } else { (1.0 / ray_dy).abs() };

        let (step_x, mut sdx) = if ray_dx < 0.0 {
            (-1_i32, (player.x - map_x as f32) * ddx)
        } else {
            (1, (map_x as f32 + 1.0 - player.x) * ddx)
        };
        let (step_y, mut sdy) = if ray_dy < 0.0 {
            (-1_i32, (player.y - map_y as f32) * ddy)
        } else {
            (1, (map_y as f32 + 1.0 - player.y) * ddy)
        };

        let side = loop {
            if sdx < sdy {
                sdx += ddx;
                map_x += step_x;
                if map.wall_at(map_x, map_y) { break 0u8; }
            } else {
                sdy += ddy;
                map_y += step_y;
                if map.wall_at(map_x, map_y) { break 1u8; }
            }
        };

        let perp = if side == 0 { sdx - ddx } else { sdy - ddy };
        z_buffer[x] = perp;

        let line_h = if perp > 0.0 { (SCREEN_H as f32 / perp) as i32 } else { SCREEN_H as i32 };
        col_y_start[x] = ((SCREEN_H as i32 - line_h) / 2).max(0) as usize;
        col_y_end[x]   = ((SCREEN_H as i32 + line_h) / 2).min(SCREEN_H as i32) as usize;
        col_color[x]   = if side == 0 { WALL_X } else { WALL_Y };
    }

    // ── Phase 2: Row-major ceiling / wall / floor write ───────────────────────
    for y in 0..SCREEN_H {
        let row = y * SCREEN_W;
        for x in 0..SCREEN_W {
            let color = if y < col_y_start[x] { CEILING }
                        else if y < col_y_end[x] { col_color[x] }
                        else { FLOOR };
            let i = (row + x) * 4;
            frame[i..i + 4].copy_from_slice(&color);
        }
    }

    // ── Sprite pass (other players as billboards) ─────────────────────────────
    let mut sprites: Vec<&PlayerState> =
        all_players.iter().filter(|p| p.id != player.id).collect();

    // Sort front-to-back: nearest sprite claims each screen column first.
    // Farther sprites skip already-claimed columns, so each column is written
    // at most once no matter how many players overlap.
    sprites.sort_by(|a, b| {
        let da = dist2(a, player);
        let db = dist2(b, player);
        da.partial_cmp(&db).unwrap_or(std::cmp::Ordering::Equal)
    });

    let mut sprite_z_buf = [f32::INFINITY; SCREEN_W];

    for s in sprites {
        let sp_x = s.x - player.x;
        let sp_y = s.y - player.y;

        let inv = 1.0 / (plane_x * dir_y - dir_x * plane_y);
        let tx = inv * (dir_y * sp_x - dir_x * sp_y);
        let tz = inv * (-plane_y * sp_x + plane_x * sp_y);

        if tz <= 0.1 {
            continue; // behind camera
        }

        let screen_x = ((SCREEN_W as f32 / 2.0) * (1.0 + tx / tz)) as i32;
        let h = (SCREEN_H as f32 / tz).abs() as i32;
        let sy0 = ((SCREEN_H as i32 - h) / 2).max(0) as usize;
        let sy1 = ((SCREEN_H as i32 + h) / 2).min(SCREEN_H as i32) as usize;
        let sx0 = (screen_x - h / 2).max(0) as usize;
        let sx1 = (screen_x + h / 2).clamp(0, SCREEN_W as i32) as usize;

        // Column pass: claim visible columns for this sprite (O(width), cheap).
        let mut drawable = [false; SCREEN_W];
        let mut any = false;
        for sx in sx0..sx1 {
            if tz >= z_buffer[sx] { continue; }
            if sprite_z_buf[sx].is_finite() { continue; }
            sprite_z_buf[sx] = tz;
            drawable[sx] = true;
            any = true;
        }
        if !any { continue; }

        // Row-major pixel write: sequential addresses within each row.
        for sy in sy0..sy1 {
            let row = sy * SCREEN_W;
            for sx in sx0..sx1 {
                if drawable[sx] {
                    let i = (row + sx) * 4;
                    frame[i..i + 4].copy_from_slice(&SPRITE);
                }
            }
        }
    }
}

fn dist2(a: &PlayerState, b: &PlayerState) -> f32 {
    (a.x - b.x).powi(2) + (a.y - b.y).powi(2)
}

use shared::{map::Map, player::PlayerState};

use super::{SCREEN_H, SCREEN_W};

// ── Palette ───────────────────────────────────────────────────────────────────

const CEILING: [u8; 4] = [20, 20, 40, 255];
const FLOOR: [u8; 4] = [45, 30, 15, 255];
/// Walls hit on their X face (perpendicular to X axis) — brighter
const WALL_X: [u8; 4] = [200, 200, 200, 255];
/// Walls hit on their Y face — darker, creates depth illusion
const WALL_Y: [u8; 4] = [110, 110, 110, 255];
const SPRITE: [u8; 4] = [220, 70, 70, 255];

// ── Public entry point ────────────────────────────────────────────────────────

pub fn render(frame: &mut [u8], player: &PlayerState, all_players: &[PlayerState], map: &Map) {
    // Direction vector and perpendicular camera plane (length 0.66 ≈ 66° FOV)
    let dir_x = player.angle.cos();
    let dir_y = player.angle.sin();
    let plane_x = -dir_y * 0.66;
    let plane_y = dir_x * 0.66;

    let mut z_buffer = [f32::INFINITY; SCREEN_W];

    // ── Wall pass (DDA raycasting) ────────────────────────────────────────────
    for x in 0..SCREEN_W {
        let camera_x = 2.0 * x as f32 / SCREEN_W as f32 - 1.0; // -1..1
        let ray_dx = dir_x + plane_x * camera_x;
        let ray_dy = dir_y + plane_y * camera_x;

        let mut map_x = player.x as i32;
        let mut map_y = player.y as i32;

        // How far along the ray we travel to cross one tile boundary
        let ddx = if ray_dx == 0.0 { f32::INFINITY } else { (1.0 / ray_dx).abs() };
        let ddy = if ray_dy == 0.0 { f32::INFINITY } else { (1.0 / ray_dy).abs() };

        // Initial side distances + step direction
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

        // DDA: advance until we hit a wall; track which face was hit
        let side = loop {
            if sdx < sdy {
                sdx += ddx;
                map_x += step_x;
                if map.wall_at(map_x, map_y) {
                    break 0u8; // X face
                }
            } else {
                sdy += ddy;
                map_y += step_y;
                if map.wall_at(map_x, map_y) {
                    break 1u8; // Y face
                }
            }
        };

        // Perpendicular distance avoids the fisheye effect
        let perp = if side == 0 { sdx - ddx } else { sdy - ddy };
        z_buffer[x] = perp;

        let line_h = if perp > 0.0 { (SCREEN_H as f32 / perp) as i32 } else { SCREEN_H as i32 };
        let y_start = ((SCREEN_H as i32 - line_h) / 2).max(0) as usize;
        let y_end = ((SCREEN_H as i32 + line_h) / 2).min(SCREEN_H as i32) as usize;

        let wall_color = if side == 0 { WALL_X } else { WALL_Y };

        for y in 0..SCREEN_H {
            let color = if y < y_start { CEILING } else if y < y_end { wall_color } else { FLOOR };
            let idx = (y * SCREEN_W + x) * 4;
            frame[idx..idx + 4].copy_from_slice(&color);
        }
    }

    // ── Sprite pass (other players as billboards) ─────────────────────────────
    let mut sprites: Vec<&PlayerState> =
        all_players.iter().filter(|p| p.id != player.id).collect();

    // Sort back-to-front so closer sprites overdraw farther ones
    sprites.sort_by(|a, b| {
        let da = dist2(a, player);
        let db = dist2(b, player);
        db.partial_cmp(&da).unwrap_or(std::cmp::Ordering::Equal)
    });

    for s in sprites {
        let sp_x = s.x - player.x;
        let sp_y = s.y - player.y;

        // Transform sprite into camera space
        let inv = 1.0 / (plane_x * dir_y - dir_x * plane_y);
        let tx = inv * (dir_y * sp_x - dir_x * sp_y);
        let tz = inv * (-plane_y * sp_x + plane_x * sp_y);

        if tz <= 0.1 {
            continue; // behind camera
        }

        let screen_x = ((SCREEN_W as f32 / 2.0) * (1.0 + tx / tz)) as i32;
        let h = (SCREEN_H as f32 / tz).abs() as i32;
        let sy0 = ((SCREEN_H as i32 - h) / 2).max(0);
        let sy1 = ((SCREEN_H as i32 + h) / 2).min(SCREEN_H as i32);
        let sx0 = (screen_x - h / 2).max(0);
        let sx1 = (screen_x + h / 2).min(SCREEN_W as i32);

        for sx in sx0..sx1 {
            if tz >= z_buffer[sx as usize] {
                continue; // behind a wall
            }
            for sy in sy0..sy1 {
                let idx = (sy as usize * SCREEN_W + sx as usize) * 4;
                frame[idx..idx + 4].copy_from_slice(&SPRITE);
            }
        }
    }
}

fn dist2(a: &PlayerState, b: &PlayerState) -> f32 {
    (a.x - b.x).powi(2) + (a.y - b.y).powi(2)
}

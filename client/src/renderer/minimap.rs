use shared::{map::Map, player::PlayerState};

use super::{SCREEN_H, SCREEN_W};

const TILE_PX: usize = 4; // pixels per map tile
const MARGIN: usize = 4; // offset from top-left corner
const DOT: usize = 3; // player dot size

const C_WALL: [u8; 4] = [180, 180, 180, 255];
const C_FLOOR: [u8; 4] = [35, 35, 35, 255];
const C_SELF: [u8; 4] = [0, 220, 60, 255];
const C_OTHER: [u8; 4] = [220, 80, 0, 255];

pub fn render(frame: &mut [u8], map: &Map, players: &[PlayerState], local_id: u8) {
    // ── Tiles ─────────────────────────────────────────────────────────────────
    for ty in 0..map.height {
        for tx in 0..map.width {
            let color = if map.wall_at(tx as i32, ty as i32) { C_WALL } else { C_FLOOR };
            for dy in 0..TILE_PX {
                for dx in 0..TILE_PX {
                    put(frame, MARGIN + tx * TILE_PX + dx, MARGIN + ty * TILE_PX + dy, color);
                }
            }
        }
    }

    // ── Players ───────────────────────────────────────────────────────────────
    for p in players {
        let color = if p.id == local_id { C_SELF } else { C_OTHER };
        let cx = MARGIN + (p.x * TILE_PX as f32) as usize;
        let cy = MARGIN + (p.y * TILE_PX as f32) as usize;

        // Dot
        let half = DOT / 2;
        for dy in 0..DOT {
            for dx in 0..DOT {
                put(frame, cx.saturating_sub(half) + dx, cy.saturating_sub(half) + dy, color);
            }
        }

        // Direction indicator (short line in facing direction)
        for i in 1..=5i32 {
            let lx = cx as i32 + (p.angle.cos() * i as f32) as i32;
            let ly = cy as i32 + (p.angle.sin() * i as f32) as i32;
            if lx >= 0 && lx < SCREEN_W as i32 && ly >= 0 && ly < SCREEN_H as i32 {
                put(frame, lx as usize, ly as usize, color);
            }
        }
    }
}

#[inline]
fn put(frame: &mut [u8], x: usize, y: usize, color: [u8; 4]) {
    if x < SCREEN_W && y < SCREEN_H {
        let i = (y * SCREEN_W + x) * 4;
        frame[i..i + 4].copy_from_slice(&color);
    }
}

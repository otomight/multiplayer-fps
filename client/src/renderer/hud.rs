use super::{SCREEN_H, SCREEN_W};

// ── Minimal 5×7 bitmap font ───────────────────────────────────────────────────
// Each entry: (ascii_byte, [row_bitmask × 7]).
// Bit 4 is the leftmost pixel of a 5-pixel-wide glyph.

const FONT: &[(u8, [u8; 7])] = &[
    (b'0', [0b11111, 0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b11111]),
    (b'1', [0b00100, 0b01100, 0b00100, 0b00100, 0b00100, 0b00100, 0b11111]),
    (b'2', [0b11110, 0b00001, 0b00001, 0b11111, 0b10000, 0b10000, 0b11111]),
    (b'3', [0b11111, 0b00001, 0b00001, 0b01111, 0b00001, 0b00001, 0b11111]),
    (b'4', [0b10001, 0b10001, 0b10001, 0b11111, 0b00001, 0b00001, 0b00001]),
    (b'5', [0b11111, 0b10000, 0b10000, 0b11110, 0b00001, 0b00001, 0b11110]),
    (b'6', [0b11111, 0b10000, 0b10000, 0b11111, 0b10001, 0b10001, 0b11111]),
    (b'7', [0b11111, 0b00001, 0b00010, 0b00100, 0b01000, 0b01000, 0b01000]),
    (b'8', [0b11111, 0b10001, 0b10001, 0b11111, 0b10001, 0b10001, 0b11111]),
    (b'9', [0b11111, 0b10001, 0b10001, 0b11111, 0b00001, 0b00001, 0b11111]),
    (b'F', [0b11111, 0b10000, 0b10000, 0b11110, 0b10000, 0b10000, 0b10000]),
    (b'P', [0b11110, 0b10001, 0b10001, 0b11110, 0b10000, 0b10000, 0b10000]),
    (b'S', [0b01111, 0b10000, 0b10000, 0b01110, 0b00001, 0b00001, 0b11110]),
    (b':', [0b00000, 0b00100, 0b00000, 0b00000, 0b00100, 0b00000, 0b00000]),
    (b' ', [0b00000; 7]),
];

const SCALE: usize = 2; // each font pixel → 2×2 screen pixels
const GLYPH_W: usize = 5;
const GLYPH_H: usize = 7;

const COLOR: [u8; 4] = [255, 220, 0, 255]; // bright yellow, easy to read on any bg

// ── Public API ────────────────────────────────────────────────────────────────

/// Draw "FPS:NNN" in the top-right corner.
pub fn draw_fps(frame: &mut [u8], fps: u32) {
    let text = format!("FPS:{fps}");
    let total_w = text.len() * (GLYPH_W + 1) * SCALE;
    let x0 = SCREEN_W.saturating_sub(total_w + 4);
    draw_str(frame, &text, x0, 4);
}

// ── Internals ─────────────────────────────────────────────────────────────────

fn draw_str(frame: &mut [u8], text: &str, x0: usize, y0: usize) {
    let mut cx = x0;
    for byte in text.bytes() {
        if let Some((_, bitmap)) = FONT.iter().find(|(c, _)| *c == byte) {
            draw_glyph(frame, bitmap, cx, y0);
        }
        cx += (GLYPH_W + 1) * SCALE;
    }
}

fn draw_glyph(frame: &mut [u8], bitmap: &[u8; 7], x0: usize, y0: usize) {
    for row in 0..GLYPH_H {
        for col in 0..GLYPH_W {
            if (bitmap[row] >> (GLYPH_W - 1 - col)) & 1 == 1 {
                for sy in 0..SCALE {
                    for sx in 0..SCALE {
                        let px = x0 + col * SCALE + sx;
                        let py = y0 + row * SCALE + sy;
                        if px < SCREEN_W && py < SCREEN_H {
                            let i = (py * SCREEN_W + px) * 4;
                            frame[i..i + 4].copy_from_slice(&COLOR);
                        }
                    }
                }
            }
        }
    }
}

mod hud;
mod minimap;
mod raycaster;

use crate::state::SharedState;

pub const SCREEN_W: usize = 640;
pub const SCREEN_H: usize = 480;
pub const SCREEN_WIDTH: u32 = SCREEN_W as u32;
pub const SCREEN_HEIGHT: u32 = SCREEN_H as u32;

/// Compose the full frame: 3D view + minimap overlay + HUD.
pub fn render(frame: &mut [u8], state: &SharedState, fps: u32) {
    // Start with a black canvas; the raycaster overwrites every pixel when
    // connected, but we need a clear frame for the "connecting" state too.
    frame.fill(0);

    if let Some(player) = state.local_player() {
        raycaster::render(frame, player, &state.players, &state.map);
        minimap::render(frame, &state.map, &state.players, player.id);
    } else {
        // Not connected yet — draw a dim "Connecting..." hint
        hud::draw_fps(frame, 0);
        return;
    }

    hud::draw_fps(frame, fps);
}

use shared::{map::{level, Map}, player::PlayerState};

/// Game state shared between the render loop (main thread) and network thread.
pub struct SharedState {
    pub local_id: u8,
    pub players: Vec<PlayerState>, // includes the local player
    pub map: Map,
    pub connected: bool,
}

impl SharedState {
    pub fn new() -> Self {
        Self {
            local_id: 0,
            players: Vec::new(),
            map: level(0),
            connected: false,
        }
    }

    pub fn local_player(&self) -> Option<&PlayerState> {
        self.players.iter().find(|p| p.id == self.local_id)
    }
}

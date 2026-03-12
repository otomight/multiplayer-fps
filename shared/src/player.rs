use serde::{Deserialize, Serialize};

/// State of a single player, broadcast to all clients every tick.
/// Uses a fixed-size name array to avoid heap allocation in packet hot paths.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlayerState {
    pub id: u8,
    pub name: [u8; 16], // null-padded UTF-8
    pub x: f32,
    pub y: f32,
    pub angle: f32, // radians, 0 = facing +X
}

impl PlayerState {
    pub fn new(id: u8, name: &str, x: f32, y: f32) -> Self {
        let mut name_bytes = [0u8; 16];
        let bytes = name.as_bytes();
        let len = bytes.len().min(16);
        name_bytes[..len].copy_from_slice(&bytes[..len]);
        Self {
            id,
            name: name_bytes,
            x,
            y,
            angle: 0.0,
        }
    }

    pub fn name_str(&self) -> &str {
        let end = self.name.iter().position(|&b| b == 0).unwrap_or(16);
        std::str::from_utf8(&self.name[..end]).unwrap_or("???")
    }
}

use serde::{Deserialize, Serialize};

use crate::player::PlayerState;

// ── Client → Server ──────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ClientPacket {
    /// First packet sent after connecting. Server replies with JoinAck.
    Join { username: String },

    /// Per-frame movement intent.
    /// dx/dy are tile-space deltas, da is rotation in radians.
    /// seq lets the server discard out-of-order packets.
    Input { seq: u32, dx: f32, dy: f32, da: f32 },

    Ping { seq: u32 },
    Disconnect,
}

// ── Server → Client ──────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ServerPacket {
    /// Sent once in response to Join. Carries the assigned id and which map to load.
    JoinAck { client_id: u8, map_id: u8 },

    /// Broadcast every server tick (~60 Hz) with the full list of active players.
    WorldState { seq: u32, players: Vec<PlayerState> },

    Pong { seq: u32 },
    Kicked { reason: String },
}

// ── Encode / decode helpers ───────────────────────────────────────────────────

/// Serialize a packet to bytes. Panics only if the type cannot be serialized,
/// which should never happen for our well-defined packet types.
pub fn encode<T: Serialize>(val: &T) -> Vec<u8> {
    bincode::serialize(val).expect("packet serialization failed")
}

/// Deserialize bytes into a packet. Returns None on malformed input so the
/// caller can simply drop bad packets without crashing.
pub fn decode<T: serde::de::DeserializeOwned>(bytes: &[u8]) -> Option<T> {
    bincode::deserialize(bytes).ok()
}

use std::{collections::HashMap, net::SocketAddr, time::Instant};

use shared::{
    constants::{CLIENT_TIMEOUT_SECS, MAX_CLIENTS},
    map::{level, Map},
    physics::try_move,
    player::PlayerState,
    protocol::ServerPacket,
};

// ── Per-client data ───────────────────────────────────────────────────────────

/// Last movement intent received from a client.
/// Kept between ticks so the player slides smoothly even under packet loss.
/// The client sends (0, 0, 0) when no keys are held.
#[derive(Default, Clone, Copy)]
pub struct PendingInput {
    pub seq: u32,
    pub dx: f32,
    pub dy: f32,
    pub da: f32,
}

pub struct Session {
    pub player: PlayerState,
    pub last_seen: Instant,
    pub pending: PendingInput,
}

// ── Authoritative game state ──────────────────────────────────────────────────

pub struct GameState {
    pub sessions: HashMap<SocketAddr, Session>,
    pub map: Map,
    next_id: u8,
    seq: u32,
}

impl GameState {
    pub fn new(map_id: u8) -> Self {
        Self {
            sessions: HashMap::new(),
            map: level(map_id),
            next_id: 0,
            seq: 0,
        }
    }

    /// Register a new client. Returns `(client_id, map_id)`.
    /// If the client is already known (reconnect), returns its existing id.
    /// Returns `None` when the server is full.
    pub fn add_client(&mut self, addr: SocketAddr, username: &str) -> Option<(u8, u8)> {
        // Reconnect: reuse existing session
        if let Some(s) = self.sessions.get_mut(&addr) {
            s.last_seen = Instant::now();
            return Some((s.player.id, self.map.id));
        }

        if self.sessions.len() >= MAX_CLIENTS {
            return None;
        }

        let id = self.next_id;
        self.next_id = self.next_id.wrapping_add(1);

        // Assign spawn point round-robin
        let spawn_idx = self.sessions.len() % self.map.spawn_points.len();
        let (sx, sy) = self.map.spawn_points[spawn_idx];

        self.sessions.insert(
            addr,
            Session {
                player: PlayerState::new(id, username, sx, sy),
                last_seen: Instant::now(),
                pending: PendingInput::default(),
            },
        );

        Some((id, self.map.id))
    }

    /// Update the pending input for a client, ignoring stale (out-of-order) packets.
    pub fn update_input(&mut self, addr: SocketAddr, seq: u32, dx: f32, dy: f32, da: f32) {
        if let Some(s) = self.sessions.get_mut(&addr) {
            s.last_seen = Instant::now();
            if seq >= s.pending.seq {
                s.pending = PendingInput { seq, dx, dy, da };
            }
        }
    }

    pub fn remove_client(&mut self, addr: SocketAddr) {
        self.sessions.remove(&addr);
    }

    /// Advance the simulation by one tick.
    /// Removes timed-out clients, applies pending inputs, resolves collisions.
    /// `dt` is the server tick period; clients send velocities (units/second)
    /// so we multiply here to get frame-rate-independent movement.
    pub fn tick(&mut self, dt: f32) {
        let now = Instant::now();

        self.sessions
            .retain(|_, s| now.duration_since(s.last_seen).as_secs() < CLIENT_TIMEOUT_SECS);

        for s in self.sessions.values_mut() {
            let (nx, ny) = try_move(&self.map, s.player.x, s.player.y,
                                    s.pending.dx * dt, s.pending.dy * dt);
            s.player.x = nx;
            s.player.y = ny;
            s.player.angle += s.pending.da * dt;
        }

        self.seq = self.seq.wrapping_add(1);
    }

    /// Snapshot the current world state as a ready-to-send packet.
    pub fn world_state(&self) -> ServerPacket {
        ServerPacket::WorldState {
            seq: self.seq,
            players: self.sessions.values().map(|s| s.player.clone()).collect(),
        }
    }
}

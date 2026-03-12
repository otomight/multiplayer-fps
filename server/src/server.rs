use std::{net::SocketAddr, sync::Arc, time::Duration};

use tokio::{net::UdpSocket, sync::RwLock};

use shared::{
    constants::TICK_RATE,
    protocol::{decode, encode, ClientPacket, ServerPacket},
};

use crate::state::GameState;

const MAX_PACKET: usize = 512;

// ── Public entry point ────────────────────────────────────────────────────────

pub async fn run_addr(addr: SocketAddr, map_id: u8) {
    let socket = Arc::new(UdpSocket::bind(addr).await.expect("failed to bind UDP socket"));
    let state = Arc::new(RwLock::new(GameState::new(map_id)));

    println!("Server listening on {addr} (level {map_id})");

    tokio::spawn(tick_loop(socket.clone(), state.clone()));

    recv_loop(socket, state).await;
}

// ── Tick loop (60 Hz) ─────────────────────────────────────────────────────────

async fn tick_loop(socket: Arc<UdpSocket>, state: Arc<RwLock<GameState>>) {
    let dt = 1.0_f32 / TICK_RATE as f32;
    let period = Duration::from_nanos(1_000_000_000 / TICK_RATE);
    let mut interval = tokio::time::interval(period);

    loop {
        interval.tick().await;

        // Hold the lock only long enough to advance state and snapshot packets.
        // Release before any I/O to avoid blocking the recv loop.
        let (packet_bytes, addrs) = {
            let mut gs = state.write().await;
            gs.tick(dt);
            let bytes = encode(&gs.world_state());
            let addrs: Vec<SocketAddr> = gs.sessions.keys().copied().collect();
            (bytes, addrs)
        };

        for addr in addrs {
            let _ = socket.send_to(&packet_bytes, addr).await;
        }
    }
}

// ── Receive loop ──────────────────────────────────────────────────────────────

async fn recv_loop(socket: Arc<UdpSocket>, state: Arc<RwLock<GameState>>) {
    let mut buf = [0u8; MAX_PACKET];

    loop {
        let (len, addr) = match socket.recv_from(&mut buf).await {
            Ok(v) => v,
            Err(e) => {
                eprintln!("recv error: {e}");
                continue;
            }
        };

        let Some(packet) = decode::<ClientPacket>(&buf[..len]) else {
            eprintln!("bad packet from {addr}, ignoring");
            continue;
        };

        // Process the packet and optionally produce a response.
        // The lock is held only during synchronous work, never across an await.
        let response: Option<Vec<u8>> = {
            let mut gs = state.write().await;
            match packet {
                ClientPacket::Join { username } => match gs.add_client(addr, &username) {
                    Some((id, map_id)) => {
                        println!("{addr} joined as \"{username}\" (id={id})");
                        Some(encode(&ServerPacket::JoinAck { client_id: id, map_id }))
                    }
                    None => {
                        println!("{addr} tried to join but server is full");
                        Some(encode(&ServerPacket::Kicked { reason: "server full".into() }))
                    }
                },
                ClientPacket::Input { seq, dx, dy, da } => {
                    gs.update_input(addr, seq, dx, dy, da);
                    None
                }
                ClientPacket::Ping { seq } => Some(encode(&ServerPacket::Pong { seq })),
                ClientPacket::Disconnect => {
                    println!("{addr} disconnected");
                    gs.remove_client(addr);
                    None
                }
            }
        }; // lock released here

        if let Some(bytes) = response {
            let _ = socket.send_to(&bytes, addr).await;
        }
    }
}

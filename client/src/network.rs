use std::{net::SocketAddr, sync::{Arc, RwLock}, time::Duration};

use crossbeam_channel::Receiver;
use tokio::{net::UdpSocket, time};

use shared::{
    map::level,
    protocol::{decode, encode, ClientPacket, ServerPacket},
};

use crate::state::SharedState;

/// Spawn the UDP network thread. Returns immediately; networking runs in the background.
pub fn spawn(
    server_addr: SocketAddr,
    username: String,
    state: Arc<RwLock<SharedState>>,
    input_rx: Receiver<ClientPacket>,
) {
    std::thread::spawn(move || {
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap()
            .block_on(run(server_addr, username, state, input_rx));
    });
}

async fn run(
    server_addr: SocketAddr,
    username: String,
    state: Arc<RwLock<SharedState>>,
    input_rx: Receiver<ClientPacket>,
) {
    let socket = UdpSocket::bind("0.0.0.0:0")
        .await
        .expect("failed to bind client UDP socket");
    socket.connect(server_addr).await.expect("failed to connect to server");

    // ── Join handshake with retry ─────────────────────────────────────────────
    let join_bytes = encode(&ClientPacket::Join { username });
    let mut buf = [0u8; 512];
    let mut retry = time::interval(Duration::from_secs(1));

    socket.send(&join_bytes).await.ok();

    loop {
        tokio::select! {
            result = socket.recv(&mut buf) => {
                let Ok(len) = result else { continue };
                let Some(pkt) = decode::<ServerPacket>(&buf[..len]) else { continue };
                match pkt {
                    ServerPacket::JoinAck { client_id, map_id } => {
                        let mut s = state.write().unwrap();
                        s.local_id = client_id;
                        s.map = level(map_id);
                        s.connected = true;
                        break;
                    }
                    ServerPacket::Kicked { reason } => {
                        eprintln!("Server refused connection: {reason}");
                        std::process::exit(1);
                    }
                    _ => {}
                }
            }
            _ = retry.tick() => {
                socket.send(&join_bytes).await.ok();
            }
        }
    }

    // ── Main loop: receive WorldState, drain and send inputs ──────────────────
    // Input packets are sent at the same rate the render loop produces them (~60 Hz).
    let mut send_tick = time::interval(Duration::from_millis(16));

    loop {
        tokio::select! {
            result = socket.recv(&mut buf) => {
                let Ok(len) = result else { continue };
                let Some(pkt) = decode::<ServerPacket>(&buf[..len]) else { continue };
                match pkt {
                    ServerPacket::WorldState { players, .. } => {
                        state.write().unwrap().players = players;
                    }
                    ServerPacket::Kicked { reason } => {
                        eprintln!("Kicked: {reason}");
                        std::process::exit(1);
                    }
                    _ => {}
                }
            }
            _ = send_tick.tick() => {
                while let Ok(pkt) = input_rx.try_recv() {
                    let _ = socket.send(&encode(&pkt)).await;
                }
            }
        }
    }
}

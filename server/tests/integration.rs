/// Integration tests: spin up a real server in a background task and exercise
/// the full UDP round-trip using std::net::UdpSocket (blocking, no tokio needed
/// on the client side).

use std::{
    net::UdpSocket,
    time::Duration,
};

use shared::protocol::{decode, encode, ClientPacket, ServerPacket};

// ── helpers ───────────────────────────────────────────────────────────────────

/// Start a server on a random OS-assigned port and return the bound address.
fn start_server() -> std::net::SocketAddr {
    // Bind to port 0 to let the OS pick a free port
    let addr: std::net::SocketAddr = "127.0.0.1:0".parse().unwrap();

    // We need to know the actual port before the async server binds, so we
    // bind a temporary socket to reserve the port, read the address, close it,
    // then tell the server to use that address.
    let tmp = std::net::UdpSocket::bind(addr).unwrap();
    let server_addr = tmp.local_addr().unwrap();
    drop(tmp); // release it for the server to bind

    std::thread::spawn(move || {
        tokio::runtime::Runtime::new()
            .unwrap()
            .block_on(server::run_addr(server_addr, 0))
    });

    // Give the server a moment to bind
    std::thread::sleep(Duration::from_millis(50));
    server_addr
}

/// Send one packet and wait up to 500 ms for a reply.
fn roundtrip(sock: &UdpSocket, packet: &ClientPacket) -> ServerPacket {
    sock.send(&encode(packet)).expect("send failed");
    let mut buf = [0u8; 512];
    let len = sock.recv(&mut buf).expect("recv failed");
    decode::<ServerPacket>(&buf[..len]).expect("bad packet from server")
}

// ── tests ─────────────────────────────────────────────────────────────────────

#[test]
fn join_receives_ack() {
    let server_addr = start_server();

    let sock = UdpSocket::bind("127.0.0.1:0").unwrap();
    sock.connect(server_addr).unwrap();
    sock.set_read_timeout(Some(Duration::from_millis(500))).unwrap();

    let reply = roundtrip(&sock, &ClientPacket::Join { username: "Alice".into() });

    match reply {
        ServerPacket::JoinAck { client_id, map_id } => {
            assert_eq!(map_id, 0, "should start on level 0");
            println!("joined as client_id={client_id}");
        }
        other => panic!("expected JoinAck, got {other:?}"),
    }
}

#[test]
fn ping_receives_pong() {
    let server_addr = start_server();

    let sock = UdpSocket::bind("127.0.0.1:0").unwrap();
    sock.connect(server_addr).unwrap();
    sock.set_read_timeout(Some(Duration::from_millis(500))).unwrap();

    // Join first so the server knows us
    roundtrip(&sock, &ClientPacket::Join { username: "Bob".into() });

    let reply = roundtrip(&sock, &ClientPacket::Ping { seq: 42 });
    match reply {
        ServerPacket::Pong { seq } => assert_eq!(seq, 42),
        other => panic!("expected Pong, got {other:?}"),
    }
}

#[test]
fn two_clients_both_appear_in_world_state() {
    let server_addr = start_server();

    let make_client = |name: &str| {
        let sock = UdpSocket::bind("127.0.0.1:0").unwrap();
        sock.connect(server_addr).unwrap();
        sock.set_read_timeout(Some(Duration::from_millis(500))).unwrap();
        roundtrip(&sock, &ClientPacket::Join { username: name.into() });
        sock
    };

    let alice = make_client("Alice");
    let _bob = make_client("Bob");

    // Wait for at least one WorldState broadcast (server ticks at 60 Hz)
    std::thread::sleep(Duration::from_millis(100));

    // Drain WorldState packets until we get one with 2 players
    let mut buf = [0u8; 512];
    let len = alice.recv(&mut buf).unwrap();
    let packet = decode::<ServerPacket>(&buf[..len]).unwrap();

    match packet {
        ServerPacket::WorldState { players, .. } => {
            assert_eq!(players.len(), 2, "expected 2 players, got {}", players.len());
            let names: Vec<&str> = players.iter().map(|p| p.name_str()).collect();
            println!("players in world: {names:?}");
            assert!(names.contains(&"Alice"));
            assert!(names.contains(&"Bob"));
        }
        other => panic!("expected WorldState, got {other:?}"),
    }
}

#[test]
fn server_rejects_when_full() {
    use shared::constants::MAX_CLIENTS;

    let server_addr = start_server();

    let mut sockets = Vec::new();
    for i in 0..MAX_CLIENTS {
        let sock = UdpSocket::bind("127.0.0.1:0").unwrap();
        sock.connect(server_addr).unwrap();
        sock.set_read_timeout(Some(Duration::from_millis(500))).unwrap();
        roundtrip(&sock, &ClientPacket::Join { username: format!("player{i}") });
        sockets.push(sock);
    }

    // One more — should be kicked
    let late = UdpSocket::bind("127.0.0.1:0").unwrap();
    late.connect(server_addr).unwrap();
    late.set_read_timeout(Some(Duration::from_millis(500))).unwrap();
    let reply = roundtrip(&late, &ClientPacket::Join { username: "overflow".into() });

    match reply {
        ServerPacket::Kicked { reason } => println!("correctly kicked: {reason}"),
        other => panic!("expected Kicked, got {other:?}"),
    }
}

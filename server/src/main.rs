#[tokio::main]
async fn main() {
    let mut args = std::env::args().skip(1);

    let port: u16 = args
        .next()
        .and_then(|s| s.parse().ok())
        .unwrap_or(shared::constants::SERVER_PORT);

    let map_id: u8 = args
        .next()
        .and_then(|s| s.parse().ok())
        .unwrap_or(0);

    if map_id >= shared::map::LEVEL_COUNT {
        eprintln!(
            "Unknown level {map_id}. Available: 0..{}",
            shared::map::LEVEL_COUNT - 1
        );
        std::process::exit(1);
    }

    let addr = format!("0.0.0.0:{port}").parse().expect("invalid address");
    server::run_addr(addr, map_id).await;
}

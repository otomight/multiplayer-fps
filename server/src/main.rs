#[tokio::main]
async fn main() {
    let port: u16 = std::env::args()
        .nth(1)
        .and_then(|s| s.parse().ok())
        .unwrap_or(shared::constants::SERVER_PORT);

    let addr = format!("0.0.0.0:{port}").parse().expect("invalid address");
    server::run_addr(addr).await;
}

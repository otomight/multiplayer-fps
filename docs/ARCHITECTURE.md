# Multiplayer FPS (Maze Wars Clone) — Project Documentation

## Overview

A Rust implementation of [Maze Wars](https://www.youtube.com/watch?v=5V5X5SbSjns): a first-person multiplayer maze game rendered with raycasting. Players connect to a central server over UDP, navigate 3D-looking mazes, and see other players in their view.

---

## Requirements Summary

| Requirement | Details |
|---|---|
| Rendering | Raycasting (Maze Wars style), mini map, FPS counter |
| Networking | UDP client-server, ≥10 simultaneous clients |
| Levels | ≥3 mazes with increasing difficulty |
| Performance | ≥50 FPS at all times |
| Input flow | CLI prompt → IP address → username → game starts |

---

## Cargo Workspace Structure

```
multiplayer-fps/
├── Cargo.toml              ← workspace root
├── shared/                 ← protocol, map, player state (no_std compatible)
│   ├── Cargo.toml
│   └── src/
│       ├── lib.rs
│       ├── protocol.rs     ← all UDP packet types
│       ├── map.rs          ← maze tile grid + level definitions
│       └── player.rs       ← PlayerState (position, angle, id)
├── server/
│   ├── Cargo.toml
│   └── src/
│       ├── main.rs         ← entry point, arg parsing, tokio runtime
│       ├── game.rs         ← authoritative game state (Arc<Mutex<GameState>>)
│       └── network.rs      ← UDP recv/send loop, client session table
└── client/
    ├── Cargo.toml
    └── src/
        ├── main.rs         ← CLI prompts, winit event loop
        ├── network.rs      ← UDP send/recv on a background thread
        ├── state.rs        ← local game state shared between threads
        ├── input.rs        ← keyboard event → movement delta
        ├── renderer/
        │   ├── mod.rs
        │   ├── raycaster.rs  ← DDA raycasting algorithm
        │   ├── minimap.rs    ← 2D overhead map
        │   └── hud.rs        ← FPS counter, username display
        └── map.rs            ← thin wrapper re-exporting shared::map
```

### Root `Cargo.toml`

```toml
[workspace]
members = ["shared", "server", "client"]
resolver = "2"
```

---

## Crates (Dependencies)

### `shared`

| Crate | Version | Purpose |
|---|---|---|
| `serde` | `1` | Derive `Serialize`/`Deserialize` on all packet types |
| `bincode` | `2` | Compact binary serialization for UDP payloads |

### `server`

| Crate | Version | Purpose |
|---|---|---|
| `tokio` | `1` (features: `rt-multi-thread`, `net`, `sync`, `time`) | Async runtime, `UdpSocket`, `RwLock` |
| `shared` | path | Shared protocol & map types |

### `client`

| Crate | Version | Purpose |
|---|---|---|
| `winit` | `0.30` | Window creation, OS event loop, keyboard input |
| `pixels` | `0.14` | WGPU-backed pixel framebuffer for software rendering |
| `glam` | `0.28` | `Vec2`/`Vec3` math for raycasting |
| `crossbeam-channel` | `0.5` | Lock-free MPSC between network thread and render thread |
| `tokio` | `1` (features: `rt`, `net`) | Async UDP socket on network thread |
| `shared` | path | Shared protocol & map types |

> **Why `pixels` + `winit`?**
> The raycaster writes individual pixels into a CPU buffer each frame. `pixels` uploads that buffer to the GPU each frame via wgpu — zero copy overhead, works on Linux/macOS/Windows, and trivially achieves >50 FPS.

---

## Architecture

### Threading Model

```
┌─────────────────────────────────────────────────────────────────┐
│  CLIENT PROCESS                                                 │
│                                                                 │
│  Main Thread (winit event loop)                                 │
│  ┌─────────────┐    reads     ┌──────────────────────────────┐  │
│  │  Game Loop  │ ──────────▶ │  Arc<RwLock<SharedState>>    │  │
│  │  Raycaster  │              │  - local PlayerState         │  │
│  │  Minimap    │              │  - remote players Vec<...>   │  │
│  │  HUD        │              │  - current map               │  │
│  └─────────────┘              └──────────────────────────────┘  │
│         │ sends input                      ▲                    │
│         ▼                                  │ writes             │
│  ┌─────────────┐                  ┌────────────────┐           │
│  │  crossbeam  │                  │ Network Thread │           │
│  │  channel    │ ────────────▶   │  tokio runtime │           │
│  │ (OutPacket) │                  │  UDP socket    │           │
│  └─────────────┘                  └────────────────┘           │
│                                          │ UDP                  │
└──────────────────────────────────────────│──────────────────────┘
                                           │
                          ┌────────────────▼────────────────┐
                          │  SERVER PROCESS                 │
                          │  tokio multi-thread runtime     │
                          │  single UdpSocket               │
                          │  Arc<RwLock<GameState>>         │
                          │    - HashMap<ClientId, Session> │
                          │    - Vec<PlayerState>           │
                          │    - current map                │
                          └─────────────────────────────────┘
```

### UDP Packet Protocol (`shared/src/protocol.rs`)

All packets are `bincode`-serialized. Max size target: **512 bytes**.

```rust
// Client → Server
#[derive(Serialize, Deserialize)]
pub enum ClientPacket {
    Join { username: String },
    Input { seq: u32, dx: f32, dy: f32, da: f32 },  // movement delta
    Ping { seq: u32 },
    Disconnect,
}

// Server → Client
#[derive(Serialize, Deserialize)]
pub enum ServerPacket {
    JoinAck { client_id: u8, map_id: u8 },
    WorldState { seq: u32, players: Vec<PlayerState> },
    Pong { seq: u32 },
    Kicked { reason: String },
}

#[derive(Serialize, Deserialize, Clone)]
pub struct PlayerState {
    pub id: u8,
    pub name: [u8; 16],  // fixed-size, no heap alloc
    pub x: f32,
    pub y: f32,
    pub angle: f32,
}
```

### Server Game Loop (`server/src/game.rs`)

```
tokio::spawn(game_tick_loop) at 60 Hz
  for each connected client:
    apply last received input
    collide with walls
  broadcast WorldState to all clients
  drop clients silent for >5 s
```

### Raycasting Algorithm (`client/src/renderer/raycaster.rs`)

Classic DDA (Digital Differential Analysis) raycaster:

1. For each screen column `x` in `[0, SCREEN_WIDTH)`:
   - Compute ray direction from player angle + FOV
   - Step through the tile grid using DDA until a wall is hit
   - Record `distance`, `wall_side` (N/S vs E/W for shading)
2. Compute `wall_height = SCREEN_HEIGHT / distance`
3. Draw vertical stripe: ceiling color → wall stripe (shaded) → floor color
4. After all walls: draw other players as vertical sprites (billboard)
5. Render minimap overlay (top-left corner)
6. Render FPS counter (top-right corner, updated every 0.5 s)

---

## Map Format (`shared/src/map.rs`)

```rust
pub struct Map {
    pub id: u8,
    pub width: usize,
    pub height: usize,
    pub tiles: Vec<u8>,   // 0 = floor, 1+ = wall type
    pub spawn_points: Vec<(f32, f32)>,
}

pub fn level(id: u8) -> Map { ... }
// level(0) → simple 16×16 maze, few dead ends
// level(1) → 24×24 maze, more dead ends
// level(2) → 32×32 maze, complex, multiple loops
```

Mazes are hand-crafted as `const` byte arrays embedded in the binary. No file I/O required at runtime.

---

## Implementation Steps

### Step 1 — Workspace Skeleton

```bash
cargo new --lib shared
cargo new --bin server
cargo new --bin client
```

Edit root `Cargo.toml` to declare the workspace. Add crate dependencies in each member's `Cargo.toml`.

### Step 2 — Shared Crate

- Define `PlayerState`, `ClientPacket`, `ServerPacket` in `protocol.rs`
- Define `Map` struct and 3 hardcoded levels in `map.rs`
- Add `serde` derives and `bincode` encode/decode helpers

### Step 3 — Server

- Bind `UdpSocket` on a configurable port (default `34254`)
- Main recv loop: deserialize `ClientPacket`, dispatch to handler
- `JoinAck`: assign `client_id`, send current `map_id`
- `Input`: update player delta in session table
- Game tick (60 Hz `tokio::time::interval`): apply inputs, wall-collide, broadcast `WorldState`
- Session timeout: remove clients inactive >5 s

### Step 4 — Client Networking Thread

- Spawn a `std::thread` that runs a `tokio` single-thread runtime
- `UdpSocket::bind("0.0.0.0:0")` then `connect(server_addr)`
- Send `ClientPacket::Join` on startup
- Recv loop: deserialize `ServerPacket`, write into `Arc<RwLock<SharedState>>`
- Input drain loop: read from `crossbeam_channel`, send `ClientPacket::Input`

### Step 5 — Window + Event Loop

- In `main.rs`: prompt for IP and username via `print!` / `stdin().read_line()`
- Create `winit::EventLoop`, `Window` (title: "Maze Wars"), `pixels::Pixels`
- `Event::RedrawRequested` → game loop body
- `Event::WindowEvent::KeyboardInput` → update held-key bitfield
- Request redraw at end of every frame

### Step 6 — Raycaster

Implement DDA in `raycaster.rs`. Tuning parameters:

```rust
const FOV: f32 = std::f32::consts::FRAC_PI_3; // 60°
const SCREEN_WIDTH: u32 = 640;
const SCREEN_HEIGHT: u32 = 480;
const MOVE_SPEED: f32 = 3.0;   // tiles/sec
const ROT_SPEED: f32 = 2.0;    // rad/sec
```

Sprite rendering for other players: compute angle and distance to each remote player, sort back-to-front, draw a vertical stripe scaled by distance.

### Step 7 — Minimap

In `minimap.rs`: blit a downscaled tile grid (e.g. 4 px/tile) into the top-left corner of the framebuffer. Draw the local player as a colored dot with a short direction line. Draw remote players as differently-colored dots.

### Step 8 — HUD

In `hud.rs`: use a simple bitmap font (embed a 8×8 ASCII font as `const [u8; N]`) to blit the FPS string and username into the framebuffer. No external font crate needed.

### Step 9 — Input → Movement

In `input.rs`: maintain a `HashSet<VirtualKeyCode>` of pressed keys. Each frame, compute `(dx, dy, da)` from held keys × `delta_time`. Send via `crossbeam_channel` to the network thread.

### Step 10 — Integration & Tuning

- Verify ≥50 FPS with 10 connected clients (profile with `cargo flamegraph` if needed)
- Test level transitions (server sends `map_id`, client switches map)
- Check compile: `cargo clippy --all -- -D warnings`

---

## `Cargo.toml` Files (reference)

### `shared/Cargo.toml`

```toml
[package]
name = "shared"
version = "0.1.0"
edition = "2021"

[dependencies]
serde = { version = "1", features = ["derive"] }
bincode = "2"
```

### `server/Cargo.toml`

```toml
[package]
name = "server"
version = "0.1.0"
edition = "2021"

[dependencies]
shared = { path = "../shared" }
tokio = { version = "1", features = ["rt-multi-thread", "net", "sync", "time", "macros"] }
```

### `client/Cargo.toml`

```toml
[package]
name = "client"
version = "0.1.0"
edition = "2021"

[dependencies]
shared     = { path = "../shared" }
winit      = "0.30"
pixels     = "0.14"
glam       = "0.28"
crossbeam-channel = "0.5"
tokio      = { version = "1", features = ["rt", "net", "macros"] }
```

---

## Running the Project

```bash
# Terminal 1 — start server on port 34254
cargo run -p server -- --port 34254

# Terminal 2 — start a local client
cargo run -p client
# > Enter IP Address: 127.0.0.1:34254
# > Enter Name: Alice
# (game window opens)

# Terminal 3 — second client (or from another machine)
cargo run -p client
# > Enter IP Address: <server-ip>:34254
# > Enter Name: Bob
```

---

## Bonus Features (optional)

| Feature | Approach |
|---|---|
| Level editor | Add a `editor` binary crate; render the map in 2D with `pixels`, allow painting walls with mouse clicks, export to `shared::Map` |
| Procedural maze gen | Implement **recursive backtracker** or **Prim's algorithm** in `shared::map`; add a `ServerPacket::GeneratedMap { tiles: Vec<u8> }` variant |
| AI players | Server-side: simple BFS pathfinding per AI agent, injected as regular `PlayerState` entries in `WorldState` |
| GUI host history | Store `~/.maze_wars_hosts` as a plain text file; read on startup and present a numbered list before the IP prompt |

---

## Key Design Decisions

- **UDP without reliability layer**: for position updates, dropped packets are acceptable (state is re-sent every tick). Only `Join`/`JoinAck` needs retransmit logic (simple timeout + retry).
- **Server-authoritative**: clients send *inputs*, not positions. The server moves players and broadcasts state. This prevents cheating and keeps all clients in sync.
- **Software raycaster**: avoids GPU shader complexity, trivially achieves 50+ FPS at 640×480 on any modern CPU, and matches the Maze Wars aesthetic.
- **Fixed-size packet fields** (`[u8; 16]` for names): avoids heap allocation in hot paths and keeps packet size deterministic.

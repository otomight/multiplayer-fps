# multiplayer-fps — Maze Wars clone in Rust

A first-person multiplayer maze game inspired by [Maze Wars](https://www.youtube.com/watch?v=5V5X5SbSjns).
Software raycaster, UDP networking, up to 10 simultaneous players.

---

## Quick start

```bash
# Terminal 1 — start the server (default port 34254, level 0)
cargo run -p server

# Terminal 2 — start a client
cargo run -p client
# Enter IP Address: 127.0.0.1:34254
# Enter Name: Alice
# (game window opens)

# Optional: second player in Terminal 3
cargo run -p client
```

Controls: **WASD** or **arrow keys** to move and turn. **Escape** to quit.

Run all tests:

```bash
cargo test --workspace
```

---

## Server arguments

```
cargo run -p server -- [port] [level]
```

| Argument | Type | Default | Description |
|---|---|---|---|
| `port` | u16 | `34254` | UDP port to listen on |
| `level` | 0 / 1 / 2 | `0` | Map to load (0 = easy, 1 = medium, 2 = hard) |

Both arguments are positional and optional. To set a level you must also provide a port.

```bash
cargo run -p server                 # default port, level 0
cargo run -p server -- 34254 1      # level 1
cargo run -p server -- 34254 2      # level 2
cargo run -p server -- 9000 0       # custom port
```

---

## Project layout

```
multiplayer-fps/
├── shared/          Protocol, maps, physics — imported by both server and client
├── server/          Authoritative game server (UDP, 60 Hz tick)
├── client/          Game window, raycaster, networking
└── docs/
    ├── subject.md   Original project requirements
    ├── audit.md     Grading checklist
    └── ARCHITECTURE.md  Full design document
```

---

## Crate responsibilities

### `shared` — the contract

Everything both sides must agree on lives here. **Start here if you touch the protocol or maps.**

| File | What's in it |
|---|---|
| `protocol.rs` | `ClientPacket` / `ServerPacket` enums + `encode()` / `decode()` helpers |
| `player.rs` | `PlayerState` — id, name (fixed 16 bytes), x, y, angle |
| `map.rs` | `Map` struct, `wall_at()`, `level(id)` — 3 hardcoded mazes |
| `physics.rs` | `try_move()` — axis-separated wall collision |
| `constants.rs` | `MOVE_SPEED`, `ROT_SPEED`, `SERVER_PORT`, `MAX_CLIENTS`, `TICK_RATE` … |

### `server`

| File | What's in it |
|---|---|
| `main.rs` | Entry point — optional `[port] [level]` arguments |
| `server.rs` | `recv_loop` (packet dispatch) + `tick_loop` (60 Hz broadcast) |
| `state.rs` | `GameState` — session table, `add_client`, `update_input`, `tick` |
| `tests/integration.rs` | 4 integration tests (real UDP, no mocks) |

### `client`

| File | What's in it |
|---|---|
| `main.rs` | CLI prompts, connection wait, winit event loop |
| `state.rs` | `SharedState` — local id, player list, current map |
| `network.rs` | Background tokio thread: Join handshake, WorldState updates |
| `input.rs` | `KeyState` — tracks held keys, `compute_input()` → (dx, dy, da) |
| `renderer/mod.rs` | `render()` — calls raycaster → minimap → HUD in order |
| `renderer/raycaster.rs` | DDA raycasting: walls + billboard sprites + Z-buffer |
| `renderer/minimap.rs` | 2D tile grid overlay, player dots, direction lines |
| `renderer/hud.rs` | Bitmap FPS counter (5×7 font, top-right corner) |

---

## Data flow

```
User keyboard
     │ winit KeyEvent
     ▼
input::KeyState::compute_input()
     │ (dx, dy, da)
     ▼
crossbeam channel ──► network thread
                           │ ClientPacket::Input  (UDP)
                           ▼
                       SERVER
                       state.tick()
                           │ try_move + collision
                           ▼
                       ServerPacket::WorldState  (UDP, 60 Hz)
                           │
                       network thread
                           │ writes Vec<PlayerState>
                           ▼
                       Arc<RwLock<SharedState>>
                           │ read each frame
                           ▼
                       renderer::render()
                           │
                       softbuffer → screen
```

---

## Threading model

| Thread | Role |
|---|---|
| Main (winit) | Event loop, raycaster, minimap, HUD — runs at monitor refresh rate |
| Network (tokio) | UDP send/recv — decoupled from render via `Arc<RwLock<>>` and `crossbeam_channel` |

The lock is held for reads only during rendering and never across an `await` point in the server.

---

## Key design decisions

**Server-authoritative.** Clients send input (dx, dy, da), not positions. The server runs `try_move` before applying them. This prevents desync and makes cheating harder.

**`[u8; 16]` for player names.** Fixed-size so `PlayerState` is `Copy`, no heap allocation in the 60 Hz broadcast hot path.

**`try_move` in `shared`.** Both server (authoritative) and client (future prediction) must run identical collision logic. One source of truth.

**Software raycaster.** Matches Maze Wars aesthetics, trivially exceeds 50 FPS at 640×480 on any modern CPU.

**`softbuffer` over `pixels`.** `pixels` (wgpu-backed) requires a `'static` window handle which creates self-referential struct problems in winit 0.30's `ApplicationHandler`. `softbuffer` takes owned `Arc<Window>` and has no lifetime issues.

---

## Adding a new level

1. Open `shared/src/map.rs`.
2. Add a new `const LEVEL_N: &[&str]` — same rules as existing levels: `1` = wall, `0` = floor, all borders must be walls.
3. Add a new arm to the `level(id)` match and increment `LEVEL_COUNT`.
4. Add a test in `map::tests` — the existing `check_level` helper validates borders and spawn points automatically.

## Changing movement speed

Edit `shared/src/constants.rs`. Both server and client import from there — one change updates both.

## Changing the protocol

1. Edit `shared/src/protocol.rs` — add a variant to `ClientPacket` or `ServerPacket`.
2. Update `server/src/server.rs` (`recv_loop` match) to handle the new client packet.
3. Update `client/src/network.rs` to handle the new server packet.
4. Update `server/tests/integration.rs` if the change affects observable behavior.

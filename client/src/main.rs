mod input;
mod network;
mod renderer;
mod state;

use std::{
    io::{BufRead, Write},
    net::SocketAddr,
    num::NonZeroU32,
    sync::{Arc, RwLock},
    time::{Duration, Instant},
};

use softbuffer::{Context, Surface};
use winit::{
    application::ApplicationHandler,
    dpi::LogicalSize,
    event::{ElementState, WindowEvent},
    event_loop::{ActiveEventLoop, EventLoop},
    keyboard::{KeyCode, PhysicalKey},
    window::{Window, WindowId},
};

use shared::protocol::ClientPacket;
use input::KeyState;
use renderer::{SCREEN_H, SCREEN_W, SCREEN_HEIGHT, SCREEN_WIDTH};
use state::SharedState;

// ── CLI helpers ───────────────────────────────────────────────────────────────

fn prompt(msg: &str) -> String {
    print!("{msg}");
    std::io::stdout().flush().unwrap();
    std::io::stdin()
        .lock()
        .lines()
        .next()
        .expect("stdin closed")
        .expect("stdin error")
        .trim()
        .to_owned()
}

// ── Application state ─────────────────────────────────────────────────────────

struct App {
    state: Arc<RwLock<SharedState>>,
    input_tx: crossbeam_channel::Sender<ClientPacket>,

    window: Option<Arc<Window>>,
    context: Option<Context<Arc<Window>>>,
    surface: Option<Surface<Arc<Window>, Arc<Window>>>,

    /// Intermediate RGBA frame buffer written by the renderer.
    rgba_buf: Vec<u8>,

    keys: KeyState,
    last_frame: Instant,
    fps: u32,
    fps_acc: f32,
    fps_frames: u32,
    input_seq: u32,
}

impl App {
    fn new(
        state: Arc<RwLock<SharedState>>,
        input_tx: crossbeam_channel::Sender<ClientPacket>,
    ) -> Self {
        Self {
            state,
            input_tx,
            window: None,
            context: None,
            surface: None,
            rgba_buf: vec![0u8; SCREEN_W * SCREEN_H * 4],
            keys: KeyState::default(),
            last_frame: Instant::now(),
            fps: 0,
            fps_acc: 0.0,
            fps_frames: 0,
            input_seq: 0,
        }
    }

    fn tick_and_render(&mut self) {
        // ── Delta time ────────────────────────────────────────────────────────
        let now = Instant::now();
        let dt = now.duration_since(self.last_frame).as_secs_f32().min(0.1);
        self.last_frame = now;

        // ── FPS (update every 0.5 s to avoid flickering) ─────────────────────
        self.fps_acc += dt;
        self.fps_frames += 1;
        if self.fps_acc >= 0.5 {
            self.fps = (self.fps_frames as f32 / self.fps_acc).round() as u32;
            self.fps_acc = 0.0;
            self.fps_frames = 0;
        }

        // ── Send movement input ───────────────────────────────────────────────
        let angle = self
            .state
            .read()
            .unwrap()
            .local_player()
            .map(|p| p.angle)
            .unwrap_or(0.0);
        let (dx, dy, da) = self.keys.compute_input(angle);
        self.input_seq += 1;
        let _ = self.input_tx.try_send(ClientPacket::Input {
            seq: self.input_seq,
            dx,
            dy,
            da,
        });

        // ── Render to RGBA buffer ─────────────────────────────────────────────
        {
            let state = self.state.read().unwrap();
            renderer::render(&mut self.rgba_buf, &state, self.fps);
        }

        // ── Blit RGBA → softbuffer u32 (0x00RRGGBB) and present ──────────────
        if let Some(surface) = &mut self.surface {
            if let Ok(mut buf) = surface.buffer_mut() {
                for (dst, rgba) in buf.iter_mut().zip(self.rgba_buf.chunks_exact(4)) {
                    *dst = ((rgba[0] as u32) << 16)
                        | ((rgba[1] as u32) << 8)
                        | rgba[2] as u32;
                }
                let _ = buf.present();
            }
        }
    }
}

// ── winit ApplicationHandler ──────────────────────────────────────────────────

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        let attrs = Window::default_attributes()
            .with_title("Maze Wars")
            .with_inner_size(LogicalSize::new(SCREEN_WIDTH, SCREEN_HEIGHT))
            .with_resizable(false);

        let window = Arc::new(
            event_loop
                .create_window(attrs)
                .expect("failed to create window"),
        );

        // softbuffer takes Arc<Window> by value — no lifetime issues
        let context = Context::new(Arc::clone(&window)).expect("failed to create softbuffer context");
        let mut surface =
            Surface::new(&context, Arc::clone(&window)).expect("failed to create softbuffer surface");

        surface
            .resize(
                NonZeroU32::new(SCREEN_WIDTH).unwrap(),
                NonZeroU32::new(SCREEN_HEIGHT).unwrap(),
            )
            .expect("failed to resize surface");

        self.window = Some(window);
        self.context = Some(context);
        self.surface = Some(surface);
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        _id: WindowId,
        event: WindowEvent,
    ) {
        match event {
            WindowEvent::CloseRequested => event_loop.exit(),

            WindowEvent::KeyboardInput { event: key_event, .. } => {
                if key_event.state == ElementState::Pressed {
                    if let PhysicalKey::Code(KeyCode::Escape) = key_event.physical_key {
                        event_loop.exit();
                        return;
                    }
                }
                self.keys.update(&key_event);
            }

            WindowEvent::RedrawRequested => {
                self.tick_and_render();
            }

            _ => {}
        }
    }

    /// Called when the OS event queue drains — drives the continuous game loop.
    fn about_to_wait(&mut self, _event_loop: &ActiveEventLoop) {
        if let Some(w) = &self.window {
            w.request_redraw();
        }
    }
}

// ── Entry point ───────────────────────────────────────────────────────────────

fn main() {
    // Audit: ask for IP then username before opening the window
    let server_addr: SocketAddr = loop {
        let raw = prompt("Enter IP Address: ");
        match raw.parse() {
            Ok(addr) => break addr,
            Err(_) => eprintln!("Invalid address (example: 127.0.0.1:34254), try again"),
        }
    };

    let username = prompt("Enter Name: ");
    println!("Starting...");

    let state = Arc::new(RwLock::new(SharedState::new()));
    let (input_tx, input_rx) = crossbeam_channel::unbounded::<ClientPacket>();

    network::spawn(server_addr, username, Arc::clone(&state), input_rx);

    // Block until JoinAck received (or timeout after 5 s)
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        if state.read().unwrap().connected {
            break;
        }
        if Instant::now() > deadline {
            eprintln!("Could not connect within 5 seconds. Is the server running?");
            std::process::exit(1);
        }
        std::thread::sleep(Duration::from_millis(50));
    }

    let event_loop = EventLoop::new().expect("failed to create event loop");
    let mut app = App::new(state, input_tx);
    event_loop.run_app(&mut app).expect("event loop error");
}

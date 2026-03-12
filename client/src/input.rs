use shared::constants::{MOVE_SPEED, ROT_SPEED};
use winit::event::{ElementState, KeyEvent};
use winit::keyboard::{KeyCode, PhysicalKey};

/// Tracks which movement keys are currently held down.
#[derive(Default)]
pub struct KeyState {
    pub forward: bool,
    pub backward: bool,
    pub turn_left: bool,
    pub turn_right: bool,
}

impl KeyState {
    /// Update state from a winit KeyEvent. Uses physical keys so WASD works
    /// regardless of keyboard layout.
    pub fn update(&mut self, event: &KeyEvent) {
        let pressed = event.state == ElementState::Pressed;
        match event.physical_key {
            PhysicalKey::Code(KeyCode::KeyW) | PhysicalKey::Code(KeyCode::ArrowUp) => {
                self.forward = pressed;
            }
            PhysicalKey::Code(KeyCode::KeyS) | PhysicalKey::Code(KeyCode::ArrowDown) => {
                self.backward = pressed;
            }
            PhysicalKey::Code(KeyCode::KeyA) | PhysicalKey::Code(KeyCode::ArrowLeft) => {
                self.turn_left = pressed;
            }
            PhysicalKey::Code(KeyCode::KeyD) | PhysicalKey::Code(KeyCode::ArrowRight) => {
                self.turn_right = pressed;
            }
            _ => {}
        }
    }

    /// Returns (dx, dy, da) — movement deltas already scaled by speed × dt,
    /// ready to put straight into a ClientPacket::Input.
    pub fn compute_input(&self, angle: f32, dt: f32) -> (f32, f32, f32) {
        let mut dx = 0.0_f32;
        let mut dy = 0.0_f32;
        let mut da = 0.0_f32;

        if self.forward {
            dx += angle.cos() * MOVE_SPEED * dt;
            dy += angle.sin() * MOVE_SPEED * dt;
        }
        if self.backward {
            dx -= angle.cos() * MOVE_SPEED * dt;
            dy -= angle.sin() * MOVE_SPEED * dt;
        }
        if self.turn_left {
            da -= ROT_SPEED * dt;
        }
        if self.turn_right {
            da += ROT_SPEED * dt;
        }

        (dx, dy, da)
    }
}

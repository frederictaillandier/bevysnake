use bevy::input::mouse::{AccumulatedMouseMotion, AccumulatedMouseScroll};
use bevy::prelude::*;

use crate::map::clip_plane::ClipPlane;

const PAN_SPEED: f32 = 20.0;
const ZOOM_SPEED: f32 = 2.0;
const CLIP_SPEED: f32 = 0.2;
const ROTATE_SPEED: f32 = 0.005;
const MIN_DISTANCE: f32 = 4.0;
const MAX_DISTANCE: f32 = 80.0;
const MIN_PITCH: f32 = 0.2;
const MAX_PITCH: f32 = std::f32::consts::FRAC_PI_2 - 0.05;

#[derive(Component)]
pub struct CameraController {
    /// point around which the camera orbits
    pub anchor: Vec3,
    pub distance: f32,
    pub yaw: f32,
    pub pitch: f32,
}

impl Default for CameraController {
    fn default() -> Self {
        Self {
            anchor: Vec3::new(0.0, 8.0, 0.0),
            distance: 24.0,
            yaw: 0.3,
            pitch: 0.9,
        }
    }
}

pub struct CameraPlugin;

impl Plugin for CameraPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, spawn)
            .add_systems(Update, (camera_rotation, pan));
    }
}

fn spawn(mut commands: Commands) {
    let cam = CameraController::default();
    let transform = orbit_transform(&cam);
    commands.spawn((Camera3d::default(), transform, cam));
}

/// Pan the camera horizontally and vertically relative to the current yaw.
fn pan(
    mut query: Query<&mut CameraController>,
    keys: Res<ButtonInput<KeyCode>>,
    time: Res<Time>,
) -> Result<(), BevyError> {
    let Ok(mut camera_ctl) = query.single_mut() else {
        return Err("no camera found".into());
    };
    let dt = time.delta_secs();
    let mut pan = Vec2::ZERO;
    if keys.pressed(KeyCode::KeyW) || keys.pressed(KeyCode::ArrowUp) {
        pan.y += 1.0;
    }
    if keys.pressed(KeyCode::KeyS) || keys.pressed(KeyCode::ArrowDown) {
        pan.y -= 1.0;
    }
    if keys.pressed(KeyCode::KeyA) || keys.pressed(KeyCode::ArrowLeft) {
        pan.x -= 1.0;
    }
    if keys.pressed(KeyCode::KeyD) || keys.pressed(KeyCode::ArrowRight) {
        pan.x += 1.0;
    }

    if pan != Vec2::ZERO {
        let speed = PAN_SPEED * dt * (camera_ctl.distance / 10.0).sqrt();
        let forward = Vec3::new(-camera_ctl.yaw.sin(), 0.0, -camera_ctl.yaw.cos());
        let right = Vec3::new(camera_ctl.yaw.cos(), 0.0, -camera_ctl.yaw.sin());
        camera_ctl.anchor += (forward * pan.y + right * pan.x) * speed;
    }
    Ok(())
}

fn camera_rotation(
    keys: Res<ButtonInput<KeyCode>>,
    mouse_buttons: Res<ButtonInput<MouseButton>>,
    mouse_motion: Res<AccumulatedMouseMotion>,
    mouse_scroll: Res<AccumulatedMouseScroll>,
    mut query: Query<(&mut CameraController, &mut Transform)>,
    mut clip: ResMut<ClipPlane>,
) -> Result<(), BevyError> {
    let Ok((mut camera_ctl, mut camera_tfm)) = query.single_mut() else {
        return Err("no camera found".into());
    };

    // --- Rotation (right-click drag) ---
    if mouse_buttons.pressed(MouseButton::Right) {
        camera_ctl.yaw -= mouse_motion.delta.x * ROTATE_SPEED;
        camera_ctl.pitch =
            (camera_ctl.pitch + mouse_motion.delta.y * ROTATE_SPEED).clamp(MIN_PITCH, MAX_PITCH);
    }

    // --- Zoom (scroll) or clip plane (Ctrl+scroll) ---
    let ctrl = keys.pressed(KeyCode::ControlLeft) || keys.pressed(KeyCode::ControlRight);
    if ctrl {
        clip.y -= mouse_scroll.delta.y * CLIP_SPEED;
    } else {
        camera_ctl.distance = (camera_ctl.distance - mouse_scroll.delta.y * ZOOM_SPEED)
            .clamp(MIN_DISTANCE, MAX_DISTANCE);
    }

    *camera_tfm = orbit_transform(&camera_ctl);
    Ok(())
}

fn orbit_offset(cam: &CameraController) -> Vec3 {
    Vec3::new(
        cam.pitch.cos() * cam.yaw.sin(),
        cam.pitch.sin(),
        cam.pitch.cos() * cam.yaw.cos(),
    ) * cam.distance
}

fn orbit_transform(cam: &CameraController) -> Transform {
    Transform::from_translation(cam.anchor + orbit_offset(cam)).looking_at(cam.anchor, Vec3::Y)
}

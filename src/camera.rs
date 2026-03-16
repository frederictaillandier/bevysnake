use bevy::input::mouse::{AccumulatedMouseMotion, AccumulatedMouseScroll};
use bevy::picking::mesh_picking::ray_cast::{MeshRayCast, MeshRayCastSettings};
use bevy::prelude::*;

use crate::map::ClipPlane;

const PAN_SPEED: f32 = 20.0;
const ZOOM_SPEED: f32 = 2.0;
const CLIP_SPEED: f32 = 0.2;
const ROTATE_SPEED: f32 = 0.005;
const MIN_DISTANCE: f32 = 4.0;
const MAX_DISTANCE: f32 = 80.0;
const MIN_PITCH: f32 = 0.2;
const MAX_PITCH: f32 = std::f32::consts::FRAC_PI_2 - 0.05;

#[derive(Component)]
pub struct ManagementCamera {
    pub focus: Vec3,
    pub distance: f32,
    pub yaw: f32,
    pub pitch: f32,
}

impl Default for ManagementCamera {
    fn default() -> Self {
        Self {
            focus: Vec3::new(0.0, 8.0, 0.0),
            distance: 24.0,
            yaw: 0.3,
            pitch: 0.9,
        }
    }
}

pub struct CameraPlugin;

impl Plugin for CameraPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, spawn_camera)
            .add_systems(Update, drive_camera);
    }
}

fn spawn_camera(mut commands: Commands) {
    let cam = ManagementCamera::default();
    let transform = orbit_transform(&cam);
    commands.spawn((Camera3d::default(), transform, cam));

    commands.spawn((
        DirectionalLight {
            illuminance: 8000.0,
            shadows_enabled: false,
            ..default()
        },
        Transform::from_rotation(Quat::from_euler(EulerRot::XYZ, -1.0, 0.4, 0.0)),
    ));
}

fn drive_camera(
    time: Res<Time>,
    keys: Res<ButtonInput<KeyCode>>,
    mouse_buttons: Res<ButtonInput<MouseButton>>,
    mouse_motion: Res<AccumulatedMouseMotion>,
    mouse_scroll: Res<AccumulatedMouseScroll>,
    mut ray_cast: MeshRayCast,
    mut query: Query<(&mut ManagementCamera, &mut Transform)>,
    mut clip: ResMut<ClipPlane>,
) {
    let Ok((mut cam, mut transform)) = query.single_mut() else { return };
    let dt = time.delta_secs();

    // --- On right-click press: find terrain point along the camera look direction ---
    // The look direction goes from the camera position toward the current focus.
    if mouse_buttons.just_pressed(MouseButton::Right) {
        let camera_pos = cam_position(&cam);
        let look_dir   = (cam.focus - camera_pos).normalize();
        let ray        = Ray3d::new(camera_pos, Dir3::new(look_dir).unwrap());

        let hits = ray_cast.cast_ray(ray, &MeshRayCastSettings::default());
        if let Some((_entity, hit)) = hits.first() {
            reanchor(&mut cam, hit.point);
        }
    }

    // --- Rotation (right-click drag) ---
    if mouse_buttons.pressed(MouseButton::Right) {
        cam.yaw  -= mouse_motion.delta.x * ROTATE_SPEED;
        cam.pitch = (cam.pitch + mouse_motion.delta.y * ROTATE_SPEED).clamp(MIN_PITCH, MAX_PITCH);
    }

    // --- Zoom (scroll) or clip plane (Ctrl+scroll) ---
    let ctrl = keys.pressed(KeyCode::ControlLeft) || keys.pressed(KeyCode::ControlRight);
    if ctrl {
        clip.y -= mouse_scroll.delta.y * CLIP_SPEED;
    } else {
        cam.distance = (cam.distance - mouse_scroll.delta.y * ZOOM_SPEED).clamp(MIN_DISTANCE, MAX_DISTANCE);
    }

    // --- Pan (WASD / arrows, relative to current yaw) ---
    let mut pan = Vec2::ZERO;
    if keys.pressed(KeyCode::KeyW) || keys.pressed(KeyCode::ArrowUp)    { pan.y += 1.0; }
    if keys.pressed(KeyCode::KeyS) || keys.pressed(KeyCode::ArrowDown)  { pan.y -= 1.0; }
    if keys.pressed(KeyCode::KeyA) || keys.pressed(KeyCode::ArrowLeft)  { pan.x -= 1.0; }
    if keys.pressed(KeyCode::KeyD) || keys.pressed(KeyCode::ArrowRight) { pan.x += 1.0; }

    if pan != Vec2::ZERO {
        let speed   = PAN_SPEED * dt * (cam.distance / 10.0).sqrt();
        let forward = Vec3::new(-cam.yaw.sin(), 0.0, -cam.yaw.cos());
        let right   = Vec3::new( cam.yaw.cos(), 0.0, -cam.yaw.sin());
        cam.focus  += (forward * pan.y + right * pan.x) * speed;
    }

    *transform = orbit_transform(&cam);
}

/// Reanchor the orbit to a new focus point while keeping the camera position fixed.
fn reanchor(cam: &mut ManagementCamera, new_focus: Vec3) {
    let camera_pos = cam_position(cam);
    let offset     = camera_pos - new_focus;
    let distance   = offset.length();
    if distance < MIN_DISTANCE { return; }
    cam.focus    = new_focus;
    cam.distance = distance.clamp(MIN_DISTANCE, MAX_DISTANCE);
    cam.pitch    = (offset.y / distance).asin().clamp(MIN_PITCH, MAX_PITCH);
    cam.yaw      = offset.x.atan2(offset.z);
}

/// Camera world position derived from orbit parameters.
fn cam_position(cam: &ManagementCamera) -> Vec3 {
    cam.focus + orbit_offset(cam)
}

fn orbit_offset(cam: &ManagementCamera) -> Vec3 {
    Vec3::new(
        cam.pitch.cos() * cam.yaw.sin(),
        cam.pitch.sin(),
        cam.pitch.cos() * cam.yaw.cos(),
    ) * cam.distance
}

fn orbit_transform(cam: &ManagementCamera) -> Transform {
    Transform::from_translation(cam.focus + orbit_offset(cam)).looking_at(cam.focus, Vec3::Y)
}

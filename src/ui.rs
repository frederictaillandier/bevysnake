use crate::map::clip_plane::ClipPlane;
use bevy::prelude::*;

#[derive(Component)]
struct ClipText;

#[derive(Component)]
struct FpsText;

pub struct UiPlugin;

impl Plugin for UiPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, spawn_clip_text);
        app.add_systems(Startup, spawn_fps_counter);

        app.add_systems(Update, update_clip_text);
        app.add_systems(Update, update_fps_counter);
    }
}

fn spawn_clip_text(mut commands: Commands) {
    commands.spawn((
        Text::new("Z: 8"),
        TextFont {
            font_size: 20.0,
            ..default()
        },
        TextColor(Color::WHITE),
        Node {
            position_type: PositionType::Absolute,
            top: Val::Px(32.0),
            right: Val::Px(12.0),
            ..default()
        },
        ClipText,
    ));
}

fn update_clip_text(clip: Res<ClipPlane>, mut query: Query<&mut Text, With<ClipText>>) {
    if !clip.is_changed() {
        return;
    }
    if let Ok(mut text) = query.single_mut() {
        **text = format!("Z: {:.0}", clip.y);
    }
}

fn spawn_fps_counter(mut commands: Commands) {
    commands.spawn((
        Text::new("FPS: --"),
        TextFont {
            font_size: 20.0,
            ..default()
        },
        TextColor(Color::WHITE),
        Node {
            position_type: PositionType::Absolute,
            top: Val::Px(8.0),
            right: Val::Px(12.0),
            ..default()
        },
        FpsText,
    ));
}

fn update_fps_counter(time: Res<Time>, mut query: Query<&mut Text, With<FpsText>>) {
    let fps = 1.0 / time.delta_secs();
    if let Ok(mut text) = query.single_mut() {
        **text = format!("FPS: {fps:.0}");
    }
}

use bevy::prelude::*;

mod camera;
mod map;
mod ui;

use camera::CameraPlugin;
use map::MapPlugin;
use ui::UiPlugin;

fn main() {
    App::new()
        .insert_resource(ClearColor(Color::BLACK))
        .add_plugins(DefaultPlugins)
        .add_plugins((CameraPlugin, MapPlugin, UiPlugin))
        .run();
}

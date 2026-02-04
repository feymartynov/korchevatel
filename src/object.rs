mod hit;

use bevy::prelude::*;

pub(super) fn plugin(app: &mut App) {
    app.register_type::<Object>();
    app.add_plugins(hit::plugin);
}

#[derive(Component, Reflect)]
#[reflect(Component)]
pub struct Object;

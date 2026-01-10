use avian2d::prelude::*;
use bevy::prelude::*;

use crate::camera::FocusPoint;

const SPRITE_PATH_STANDING: &str = "images/alienGreen_stand.png";

pub(super) fn plugin(app: &mut App) {
    app.register_type::<PlayerSpawnPoint>();
    app.add_observer(spawn);
}

#[derive(Component, Debug, Clone, Copy, PartialEq, Eq, Default)]
#[require(Transform, Visibility, Collider)]
pub struct Player;

#[derive(Component, Debug, Clone, Copy, PartialEq, Eq, Default, Reflect)]
#[require(Transform)]
#[reflect(Component)]
pub struct PlayerSpawnPoint;

fn spawn(
    add_player_spawn: On<Add, PlayerSpawnPoint>,
    mut commands: Commands,
    player_query: Query<Entity, With<Player>>,
    player_spawn_query: Query<&Transform, With<PlayerSpawnPoint>>,
    mut camera_query: Query<Entity, With<IsDefaultUiCamera>>,
    asset_server: Res<AssetServer>,
) {
    if !player_query.is_empty() {
        return;
    };

    let spawn_transform = player_spawn_query
        .get(add_player_spawn.event().entity)
        .expect("transform");

    let player = commands
        .spawn((
            Name::new("Player"),
            Player,
            *spawn_transform,
            Sprite {
                image: asset_server.load(SPRITE_PATH_STANDING),
                ..Default::default()
            },
        ))
        .id();

    if let Ok(camera) = camera_query.single_mut() {
        commands.entity(camera).insert(FocusPoint::new(player));
    }
}

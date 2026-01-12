use avian2d::prelude::*;
use bevy::prelude::*;
use bevy::sprite::Anchor;

use crate::movement::Direction;
use crate::movement::MovementBundle;

const SPRITE_PATH_STANDING: &str = "images/alienGreen_stand.png";

pub(super) fn plugin(app: &mut App) {
    app.register_type::<Character>();
    app.add_observer(on_insert);
    app.add_systems(Update, flip);
}

/// Персонаж
#[derive(Component, Default, Reflect)]
#[reflect(Component)]
pub struct Character;

/// Добавляет графику, физику и логику персонажа (играбельного или нет) к сущности
fn on_insert(
    inserted: On<Insert, Character>,
    mut commands: Commands,
    asset_server: Res<AssetServer>,
) {
    commands.entity(inserted.entity).insert((
        Sprite {
            image: asset_server.load(SPRITE_PATH_STANDING),
            ..Default::default()
        },
        Anchor::from(Vec2::new(0.0, -0.21)),
        RigidBody::Dynamic,
        Collider::capsule(50.0, 50.0),
        LockedAxes::ROTATION_LOCKED,
        MovementBundle::default(),
    ));
}

/// Разворот персонажа при смене направления движения
fn flip(mut q: Query<(&mut Sprite, &Direction), Changed<Direction>>) {
    for (mut sprite, direction) in q.iter_mut() {
        sprite.flip_x = matches!(direction, Direction::Left);
    }
}

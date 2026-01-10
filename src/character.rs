use avian2d::math::*;
use avian2d::prelude::*;
use bevy::prelude::*;
use bevy::sprite::Anchor;

use crate::level::Layer;
use crate::movement::Direction;
use crate::movement::MovementBundle;

const SPRITE_PATH_STANDING: &str = "images/alienGreen_stand.png";

pub(super) fn plugin(app: &mut App) {
    app.add_observer(on_insert);
    app.add_systems(Update, flip);
    app.add_systems(FixedUpdate, change_layer);
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
    let collider = Collider::capsule(50.0, 50.0);
    let mut shape_caster = collider.clone();
    shape_caster.set_scale(Vector::ONE * 0.99, 10);

    let ground_caster =
        ShapeCaster::new(shape_caster, Vector::ZERO, 0.0, Dir2::NEG_Y).with_max_distance(10.0);

    commands.entity(inserted.entity).insert((
        Sprite {
            image: asset_server.load(SPRITE_PATH_STANDING),
            ..Default::default()
        },
        Anchor::from(Vec2::new(0.0, -0.21)),
        RigidBody::Dynamic,
        collider,
        ground_caster,
        LockedAxes::ROTATION_LOCKED,
        TranslationInterpolation,
        MovementBundle::default(),
    ));
}

/// Разворот персонажа при смене направления движения
fn flip(mut q: Query<(&mut Sprite, &Direction), Changed<Direction>>) {
    for (mut sprite, direction) in q.iter_mut() {
        sprite.flip_x = matches!(direction, Direction::Left);
    }
}

/// Смена слоя
fn change_layer(
    q: Query<(Entity, &Layer), (With<Character>, Changed<Layer>)>,
    mut commands: Commands,
) {
    for (entity, layer) in q.iter() {
        // Физическое взаимодействие только с объектами текущего слоя
        commands
            .entity(entity)
            .insert(CollisionLayers::new([*layer], [*layer]));
    }
}

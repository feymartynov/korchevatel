use avian2d::prelude::*;
use bevy::prelude::*;
use bevy::sprite::Anchor;

use crate::level::Layer;
use crate::movement::Direction;
use crate::movement::MovementBundle;

const SPRITE_PATH_STANDING: &str = "images/alienGreen_stand.png";
const Z_SCALE_FACTOR: f32 = 1.1;

pub(super) fn plugin(app: &mut App) {
    app.register_type::<Character>();
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
    commands.entity(inserted.entity).insert((
        Sprite {
            image: asset_server.load(SPRITE_PATH_STANDING),
            ..Default::default()
        },
        Anchor::from(Vec2::new(0.0, -0.21)),
        RigidBody::Dynamic,
        Collider::capsule(50.0, 50.0),
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
    mut character_q: Query<
        (&Layer, &mut Transform, &mut Collider, &mut CollisionLayers),
        (With<Character>, Changed<Layer>),
    >,
) {
    for (layer, mut transform, mut collider, mut collision_layers) in character_q.iter_mut() {
        let layer_id = layer.id();

        // Меняем Z для упроядочивания рендеринга
        transform.translation.z = layer_id as f32;

        // Меняем слои взаимодействия физики
        let layer_mask = (*layer).into();
        collision_layers.memberships = layer_mask;
        collision_layers.filters = layer_mask;

        // Масштабирование для симуляции приближения
        transform.scale = Vec3::ONE + Vec3::ONE * (Z_SCALE_FACTOR - 1.0) * (layer_id - 1) as f32;
        collider.set_scale(transform.scale.truncate(), 4);
    }
}

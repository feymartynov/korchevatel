use std::cmp::Ordering;
use std::f32;

use avian2d::math::*;
use avian2d::prelude::*;
use bevy::prelude::*;
use bevy_ecs_tiled::prelude::TiledColliderOf;
use bevy_ecs_tiled::prelude::TiledMapStorage;

use crate::level::Layer;

pub(super) fn plugin(app: &mut App) {
    app.register_type::<Ground>();
    app.register_type::<Grounded>();
    app.register_type::<Direction>();
    app.register_type::<MovementInput>();
    app.register_type::<MovementSpeed>();
    app.register_type::<MaxSlopeAngle>();
    app.register_type::<ZMode>();
    app.add_message::<MovementMessage>();

    app.add_systems(
        FixedUpdate,
        (
            update_grounded,
            on_movement_messages.before(do_move),
            do_move,
        ),
    );
}

#[derive(Message)]
pub enum MovementMessage {
    Z { entity: Entity, z_direction: i8 },
}

/// Пол, чтобы стоять
#[derive(Component, Reflect)]
#[reflect(Component)]
pub struct Ground;

/// Признак нахождения на полу
#[derive(Component, Reflect)]
#[reflect(Component)]
#[component(storage = "SparseSet")]
pub struct Grounded;

/// Направление движения
#[derive(Component, Default, Clone, Copy, PartialEq, Eq, Reflect)]
#[reflect(Component)]
pub enum Direction {
    Left,
    #[default]
    Right,
}

/// Характеристики движения
#[derive(Bundle)]
pub struct MovementBundle {
    input: MovementInput,
    direction: Direction,
    speed: MovementSpeed,
    max_slope_angle: MaxSlopeAngle,
    z_mode: ZMode,
    gravity_scale: GravityScale,
}

impl MovementBundle {
    pub fn default() -> Self {
        Self {
            input: MovementInput::default(),
            direction: Direction::default(),
            speed: MovementSpeed::default(),
            max_slope_angle: MaxSlopeAngle(PI * 0.45),
            z_mode: ZMode::default(),
            gravity_scale: GravityScale::default(),
        }
    }
}

/// Текущее действие движения
#[derive(Component, Default, Reflect)]
#[reflect(Component)]
pub struct MovementInput {
    pub x_direction: Scalar,
    pub z_direction: i8,
}

/// Параметры скорости
#[derive(Component, Reflect)]
#[reflect(Component)]
pub struct MovementSpeed {
    pub acceleration: Scalar,
    pub max_velocity: Scalar,
    pub z_speed: Scalar,
}

impl Default for MovementSpeed {
    fn default() -> Self {
        Self {
            acceleration: 100.0,
            max_velocity: 200.0,
            z_speed: 200.0,
        }
    }
}

/// Максимальный угол подъёма в горку в радианах
/// Если угол больше этого, то покатится с горы вниз
#[derive(Component, Reflect)]
#[reflect(Component)]
struct MaxSlopeAngle(Scalar);

/// Режим перехода между слоями
#[derive(Component, Default, Reflect)]
#[reflect(Component)]
pub enum ZMode {
    /// Находится на своём слое
    #[default]
    Idle,
    /// Двигается на камеру или от камеры
    Moving {
        from: Layer,
        to: Layer,
        target_y: Scalar,
    },
}

/// Определение приземления
fn update_grounded(
    mut commands: Commands,
    mut query: Query<
        (Entity, &Collider, &Rotation, &Layer, Option<&MaxSlopeAngle>),
        With<MovementInput>,
    >,
    spatial_q: SpatialQuery,
) {
    for (entity, collider, rotation, layer, max_slope_angle) in &mut query {
        let maybe_shape_hit = spatial_q.cast_shape(
            collider,
            Vector::ZERO,
            rotation.as_radians(),
            Dir2::NEG_Y,
            &ShapeCastConfig::from_max_distance(10.0),
            &SpatialQueryFilter {
                mask: (*layer).into(),
                ..Default::default()
            },
        );

        let is_grounded = maybe_shape_hit
            .map(|hit| {
                if let Some(angle) = max_slope_angle {
                    (rotation * -hit.normal2).angle_to(Vector::Y).abs() <= angle.0
                } else {
                    true
                }
            })
            .unwrap_or(true);

        if is_grounded {
            commands.entity(entity).insert(Grounded);
        } else {
            commands.entity(entity).remove::<Grounded>();
        }
    }
}

fn on_movement_messages(
    mut message_reader: MessageReader<MovementMessage>,
    mut input_q: Query<&mut MovementInput>,
) {
    for message in message_reader.read() {
        match message {
            MovementMessage::Z {
                entity,
                z_direction,
            } => {
                if let Ok(mut input) = input_q.get_mut(*entity) {
                    input.z_direction = *z_direction;
                }
            }
        }
    }
}

/// Перемещение по команде
fn do_move(
    mut q: Query<(
        Entity,
        &mut MovementInput,
        &mut Direction,
        Has<Grounded>,
        &MovementSpeed,
        &mut LinearVelocity,
        &mut Layer,
        &mut ZMode,
        &mut GravityScale,
        &GlobalTransform,
        &Collider,
    )>,
    map_storage_q: Query<&TiledMapStorage>,
    spatial_q: SpatialQuery,
    ground_collider_q: Query<&TiledColliderOf>,
    ground_q: Query<(), With<Ground>>,
    mut commands: Commands,
) {
    let Ok((
        entity,
        mut input,
        mut direction,
        is_grounded,
        speed,
        mut linear_velocity,
        mut layer,
        mut z_mode,
        mut gravity_scale,
        global_transform,
        collider,
    )) = q.single_mut()
    else {
        return;
    };

    match *z_mode {
        ZMode::Idle => {
            // Движение в стороны возможно только, когда не перемещаемся между слоями
            if is_grounded {
                linear_velocity.x += input.x_direction * speed.acceleration;

                linear_velocity.x = linear_velocity
                    .x
                    .clamp(-speed.max_velocity, speed.max_velocity);

                *direction = if linear_velocity.x < 0.0 {
                    Direction::Left
                } else {
                    Direction::Right
                };
            }
        }
        ZMode::Moving { from, to, target_y } => {
            // Завершение перемещения между слоями
            let current_y = global_transform.translation().y;

            if to < from && current_y >= target_y || to > from && current_y <= target_y {
                linear_velocity.y = 0.0;
                *layer = to;
                commands.entity(entity).remove::<ColliderDisabled>();
                *gravity_scale = GravityScale::default();
                *z_mode = ZMode::Idle;
            }
        }
    }

    // Начало перехода между слоями, если была команда
    let z_direction = std::mem::take(&mut input.z_direction);

    if !is_grounded {
        return;
    }

    let new_layer = match z_direction.cmp(&0) {
        Ordering::Equal => return,
        Ordering::Greater => layer.next(),
        Ordering::Less => layer.prev(),
    };

    let top_layer = Layer::new(
        map_storage_q
            .single()
            .expect("map storage")
            .layers()
            .count() as u32
            - 1,
    );

    if !(Layer::BOTTOM..=top_layer).contains(&new_layer) {
        return;
    }

    // Симуляция движения по Z – это на самом деле это движение по Y.
    // Чтобы узнать, нет ли препятствий, и уровень пола, узнаём, во что на целевом слое врежется
    // объект при перемещении по Y.
    let Some(shape_hit) = spatial_q.cast_shape(
        collider,
        global_transform.translation().truncate(),
        0.0,
        Dir2::from_xy(linear_velocity.x, speed.z_speed * -z_direction as f32).expect("direction"),
        // Слои не должны быть далеко. Ограничиваем зону поиска для предотвращения глюков
        &ShapeCastConfig::from_max_distance(300.0),
        &SpatialQueryFilter {
            mask: new_layer.into(),
            ..Default::default()
        },
    ) else {
        return;
    };

    // Если врежемся не в землю, значит это препятствие и смена слоя запрещена
    //
    // У объектов карты коллайдер вешается не на сам объект, а на дочку с TiledColliderOf,
    // поэтому надо сначала сходить по ссылке.
    if ground_collider_q
        .get(shape_hit.entity)
        .and_then(|tiled_collider_of| ground_q.get(tiled_collider_of.0))
        .is_err()
    {
        return;
    }

    let target_y =
        shape_hit.point1.y * shape_hit.normal1.y + shape_hit.point2.y * shape_hit.normal2.y;

    *z_mode = ZMode::Moving {
        from: *layer,
        to: new_layer,
        target_y,
    };

    // Временно отключаем проверки на столкновения и гравитацию
    commands.entity(entity).insert(ColliderDisabled);
    gravity_scale.0 = 0.0;
    linear_velocity.y -= speed.z_speed * z_direction as f32;
}

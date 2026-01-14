use std::cmp::Ordering;
use std::f32;

use avian2d::math::*;
use avian2d::prelude::*;
use bevy::ecs::entity::EntityHashSet;
use bevy::prelude::*;
use bevy_ecs_tiled::prelude::TiledColliderOf;
use bevy_ecs_tiled::prelude::TiledMapStorage;

use crate::level::Layer;

/// Во сколько раз масштабируется объект при переходе на следующий слой
const Z_SCALE_FACTOR: f32 = 1.1;
/// Максимальное кол-во проверок на столкновение при смене слоя.
/// Много — дорого. Мало — проскочит и застрянет в текстурах.
const MAX_Z_HITS: usize = 2;

pub(super) fn plugin(app: &mut App) {
    app.register_type::<Ground>();
    app.register_type::<Grounded>();
    app.register_type::<Direction>();
    app.register_type::<MovementInput>();
    app.register_type::<MovementSpeed>();
    app.register_type::<MaxSlopeAngle>();
    app.register_type::<ZMoving>();
    app.add_message::<MovementMessage>();

    app.add_systems(
        FixedUpdate,
        (
            update_grounded.before(do_move),
            on_movement_messages.before(do_move),
            do_move,
            on_layer_changed.after(do_move),
        ),
    );
}

/// Команды перемещения, которые могут быть назначены не в текущем кадре
#[derive(Message)]
pub enum MovementMessage {
    /// Перемещение объекта на 1 слой вглубь (-1) или наружу (+1)
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
    gravity_scale: GravityScale,
    transform_interpolation: TransformInterpolation,
}

impl MovementBundle {
    pub fn default() -> Self {
        Self {
            input: MovementInput::default(),
            direction: Direction::default(),
            speed: MovementSpeed::default(),
            max_slope_angle: MaxSlopeAngle(PI * 0.45),
            gravity_scale: GravityScale::default(),
            transform_interpolation: TransformInterpolation,
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

/// Переход, между слоями
/// Этот компонент добавляется только на время перехода
#[derive(Component, Reflect)]
#[reflect(Component)]
pub struct ZMoving {
    from_layer: Layer,
    to_layer: Layer,
    to_y: Scalar,
    scale_velocity: Scalar,
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
    q: Query<(
        Entity,
        &mut MovementInput,
        &mut Direction,
        Has<Grounded>,
        &MovementSpeed,
        &mut LinearVelocity,
        &mut Layer,
        Option<&ZMoving>,
        &mut GravityScale,
        &mut Transform,
        &GlobalTransform,
        &Collider,
    )>,
    map_storage_q: Query<&TiledMapStorage>,
    spatial_q: SpatialQuery,
    ground_collider_q: Query<&TiledColliderOf>,
    ground_q: Query<(), With<Ground>>,
    physics_time: Res<Time<Physics>>,
    mut commands: Commands,
) {
    'outer: for (
        entity,
        mut input,
        mut direction,
        is_grounded,
        speed,
        mut linear_velocity,
        mut layer,
        z_moving,
        mut gravity_scale,
        mut transform,
        global_transform,
        collider,
    ) in q
    {
        if let Some(&ZMoving {
            from_layer,
            to_layer,
            to_y,
            scale_velocity,
            ..
        }) = z_moving.as_ref()
        {
            // Завершение перемещения между слоями
            let current_y = global_transform.translation().y;

            if to_layer < from_layer && current_y >= *to_y
                || to_layer > from_layer && current_y <= *to_y
            {
                *layer = *to_layer;
                linear_velocity.y = 0.0;

                commands
                    .entity(entity)
                    .remove::<(ZMoving, ColliderDisabled)>();

                *gravity_scale = GravityScale::default();
                continue;
            }

            // Анимация масштабирования для симуляции перспективы
            transform.scale += scale_velocity * physics_time.delta_secs();
        } else if is_grounded {
            // Движение в стороны возможно только, когда не перемещаемся между слоями и не падаем.
            // Однако, на время перемещения скорость по X сохраняется.
            linear_velocity.x += input.x_direction * speed.acceleration;

            linear_velocity.x = linear_velocity
                .x
                .clamp(-speed.max_velocity, speed.max_velocity);

            if linear_velocity.x < -f32::EPSILON {
                *direction = Direction::Left;
            } else if linear_velocity.x > f32::EPSILON {
                *direction = Direction::Right;
            }
        }

        // Забираем команду смены слоя
        let z_direction = std::mem::take(&mut input.z_direction);

        // Нельзя менять слои в падении
        if !is_grounded {
            continue;
        }

        // Начало перехода между слоями, если была команда
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
            continue;
        }

        // Симуляция движения по Z – это на самом деле это движение по Y.
        // Чтобы узнать уровень пола и нет ли препятствий, узнаём, во что на целевом слое врежется
        // объект при перемещении по Y.
        let mut ground_shape_hit = None;

        let mut filter = SpatialQueryFilter {
            mask: new_layer.into(),
            excluded_entities: EntityHashSet::with_capacity(MAX_Z_HITS),
        };

        for _ in 0..MAX_Z_HITS {
            let Some(shape_hit) = spatial_q.cast_shape(
                collider,
                global_transform.translation().truncate(),
                0.0,
                Dir2::from_xy(linear_velocity.x, speed.z_speed * -z_direction as f32)
                    .expect("direction"),
                // Слои не должны быть далеко. Ограничиваем зону поиска для предотвращения глюков
                &ShapeCastConfig::from_max_distance(300.0),
                &filter,
            ) else {
                break;
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
                continue 'outer;
            }

            // Нашли пол, но на нём может что-то ещё стоять, поэтому проверяем дальше.
            ground_shape_hit = Some(shape_hit);
            filter.excluded_entities.insert(shape_hit.entity);
        }

        let Some(shape_hit) = ground_shape_hit else {
            continue;
        };

        // Верхушка пола, куда собираемся приехать
        let to_y =
            shape_hit.point1.y * shape_hit.normal1.y + shape_hit.point2.y * shape_hit.normal2.y;

        // Зная, за сколько приедем, рассчитываем скорость масштабирования
        let from_y = global_transform.translation().y;
        let z_moving_time = (from_y - to_y) / speed.z_speed;
        let scale_velocity = (Z_SCALE_FACTOR - 1.0) / z_moving_time;

        // Помечаем объект как перемещающийся по Z
        commands.entity(entity).insert(ZMoving {
            from_layer: *layer,
            to_layer: new_layer,
            to_y,
            scale_velocity,
        });

        // Временно отключаем проверки на столкновения и гравитацию
        commands.entity(entity).insert(ColliderDisabled);
        gravity_scale.0 = 0.0;

        // Запускаем движение по Y
        linear_velocity.y -= speed.z_speed * z_direction as f32;
    }
}

fn on_layer_changed(
    q: Query<(&Layer, &mut Transform, &mut CollisionLayers), (With<MovementInput>, Changed<Layer>)>,
) {
    for (layer, mut transform, mut collision_layers) in q {
        // Z-ordering для рендера слоёв
        let z = layer.id() as f32;
        transform.translation.z = z;
        transform.scale = Vec3::ONE + Vec3::ONE * (Z_SCALE_FACTOR - 1.0) * (z - 1.0);

        // Меняем слои взаимодействия физики
        let layer_mask = (*layer).into();
        collision_layers.memberships = layer_mask;
        // | 1, чтобы сталкивался с границами локации, находясь на любом слое
        collision_layers.filters = layer_mask | 1;
    }
}

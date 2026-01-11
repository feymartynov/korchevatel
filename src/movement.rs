use avian2d::math::*;
use avian2d::prelude::*;
use bevy::prelude::*;
use bevy_ecs_tiled::prelude::TiledMapStorage;

use crate::level::Layer;

pub(super) fn plugin(app: &mut App) {
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

/// Направление движения
#[derive(Component, Default, Clone, Copy, PartialEq, Eq)]
pub enum Direction {
    Left,
    #[default]
    Right,
}

/// Признак нахождения на полу
#[derive(Component)]
#[component(storage = "SparseSet")]
pub struct Grounded;

/// Характеристики движения
#[derive(Bundle)]
pub struct MovementBundle {
    input: MovementInput,
    direction: Direction,
    speed: MovementSpeed,
    max_slope_angle: MaxSlopeAngle,
}

#[derive(Message)]
pub enum MovementMessage {
    Z { entity: Entity, z_direction: i8 },
}

impl Default for MovementBundle {
    fn default() -> Self {
        Self {
            input: MovementInput::default(),
            direction: Direction::default(),
            speed: MovementSpeed::default(),
            max_slope_angle: MaxSlopeAngle(PI * 0.45),
        }
    }
}

/// Текущее действие движения
#[derive(Component, Default)]
pub struct MovementInput {
    pub x_direction: Scalar,
    pub z_direction: i8,
}

/// Параметры скорости
#[derive(Component, Reflect)]
pub struct MovementSpeed {
    pub acceleration: Scalar,
    pub max_velocity: Scalar,
}

impl Default for MovementSpeed {
    fn default() -> Self {
        Self {
            acceleration: 200.0,
            max_velocity: 200.0,
        }
    }
}

/// Максимальный угол подъёма в горку в радианах
/// Если угол больше этого, то покатится с горы вниз
#[derive(Component, Reflect)]
struct MaxSlopeAngle(Scalar);

/// Определение приземления
fn update_grounded(
    mut commands: Commands,
    mut query: Query<(Entity, &ShapeHits, &Rotation, Option<&MaxSlopeAngle>), With<MovementInput>>,
) {
    for (entity, hits, rotation, max_slope_angle) in &mut query {
        let is_grounded = hits.iter().any(|hit| {
            if let Some(angle) = max_slope_angle {
                (rotation * -hit.normal2).angle_to(Vector::Y).abs() <= angle.0
            } else {
                true
            }
        });

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
    mut q: Query<
        (
            &mut MovementInput,
            &mut Direction,
            &MovementSpeed,
            &mut LinearVelocity,
            &mut Layer,
        ),
        // With<Grounded>,
    >,
    map_storage_q: Query<&TiledMapStorage>,
) {
    let Ok((mut input, mut direction, speed, mut linear_velocity, mut layer)) = q.single_mut()
    else {
        return;
    };

    // Движение в стороны
    linear_velocity.x += input.x_direction * speed.acceleration;
    linear_velocity.x = linear_velocity
        .x
        .clamp(-speed.max_velocity, speed.max_velocity);

    *direction = if linear_velocity.x < 0.0 {
        Direction::Left
    } else {
        Direction::Right
    };

    // Переход между слоями
    if input.z_direction == 0 {
        return;
    }

    let current_layer_id = layer.id();
    let new_layer_id = (current_layer_id as i32 + input.z_direction as i32) as u32;
    input.z_direction = 0;

    if new_layer_id == 0
        || new_layer_id as usize
            > map_storage_q
                .single()
                .expect("map storage")
                .layers()
                .count()
                - 1
    {
        return;
    }

    *layer = Layer::new(new_layer_id);
}

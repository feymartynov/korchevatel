use avian2d::math::*;
use avian2d::prelude::*;
use bevy::prelude::*;

pub(super) fn plugin(app: &mut App) {
    app.add_systems(FixedUpdate, (update_grounded, do_move));
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
    pub direction: Scalar,
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

/// Перемещение по команде
fn do_move(
    mut controllers: Query<
        (
            &MovementInput,
            &mut Direction,
            &MovementSpeed,
            &mut LinearVelocity,
        ),
        With<Grounded>,
    >,
) {
    let Ok((input, mut direction, speed, mut linear_velocity)) = controllers.single_mut() else {
        return;
    };

    linear_velocity.x += input.direction * speed.acceleration;
    linear_velocity.x = linear_velocity.x.clamp(-speed.max_velocity, speed.max_velocity);

    let new_direction = if linear_velocity.x < 0.0 {
        Direction::Left
    } else {
        Direction::Right
    };

    // Пишем только, когда реально поменялось, чтобы не генерить лишние события.
    if new_direction != *direction {
        *direction = new_direction;
    }
}

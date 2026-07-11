use std::cmp::Ordering;

use bevy::prelude::*;

use crate::level::Layer;
use crate::movement::MovementInput;
use crate::player::Player;

use super::Mob;
use super::nav_grid::{NavGrid, PathSegment};

/// Дистанция, на которую походить к цели, чтобы не толкаться вплотную.
const TARGET_APPROACH_DISTANCE: f32 = 300.0;
/// Насколько близко нужно подойти к точке маршрута по X, чтобы считать её достигнутой.
const WAYPOINT_REACH_DISTANCE: f32 = 50.0;
/// Радиус, в котором моб чувствует присутствие игрока.
const SENSE_DISTANCE: f32 = 1_600.0;
/// Как часто перепрокладывать путь к движущемуся игроку.
const REPATH_INTERVAL_SECS: f32 = 0.35;

/// Лимит поиска пути по кол-ву рассмотренных вариантов.
const PATH_SEARCH_LIMIT: usize = 2_000;

pub(super) fn plugin(app: &mut App) {
    app.add_systems(
        FixedUpdate,
        (setup_on_mob_added, setup_on_player_added, pursue_target),
    );
}

#[derive(Component, Debug, Reflect)]
#[reflect(Component)]
struct Target(Entity);

/// Состояние преследования.
#[derive(Component, Debug, Default)]
struct Pursuit {
    path: Vec<PathSegment>,
    seconds_until_repath: f32,
}

/// Настройка при спавне моба.
fn setup_on_mob_added(
    mob_q: Query<Entity, Added<Mob>>,
    player_q: Query<Entity, With<Player>>,
    mut commands: Commands,
) {
    let maybe_player = player_q.single().ok();

    for entity in mob_q {
        commands.entity(entity).insert(Pursuit::default());

        // Если игрок заспавнен, делаем его целью.
        if let Some(player) = maybe_player {
            commands.entity(entity).insert(Target(player));
        }
    }
}

/// Настройка при спавне игрока.
fn setup_on_player_added(
    player_q: Query<Entity, Added<Player>>,
    mob_q: Query<Entity, (With<Mob>, Without<Target>)>,
    mut commands: Commands,
) {
    let Ok(player) = player_q.single() else {
        return;
    };

    // Делаем игрока целью для всех ранее заспавненных мобов.
    for entity in mob_q {
        commands.entity(entity).insert(Target(player));
    }
}

/// Ведёт моба к цели по кэшированному пути и периодически перепрокладывает маршрут.
fn pursue_target(
    time: Res<Time>,
    mut mob_q: Query<
        (
            &Target,
            &GlobalTransform,
            &Layer,
            &mut MovementInput,
            &mut Pursuit,
        ),
        With<Mob>,
    >,
    target_q: Query<(&GlobalTransform, &Layer)>,
    nav_grid_q: Query<&NavGrid>,
) {
    let Ok(nav_grid) = nav_grid_q.single() else {
        return;
    };

    for (target, mob_global_transform, mob_layer, mut movement_input, mut pursuit) in &mut mob_q {
        let Ok((target_global_transform, target_layer)) = target_q.get(target.0) else {
            continue;
        };

        update_pursuit(
            time.delta_secs(),
            mob_global_transform.translation().truncate(),
            *mob_layer,
            target_global_transform.translation().truncate(),
            *target_layer,
            nav_grid,
            &mut movement_input,
            &mut pursuit,
        );
    }
}

fn update_pursuit(
    delta_secs: f32,
    mob_position: Vec2,
    mob_layer: Layer,
    mut target_position: Vec2,
    target_layer: Layer,
    nav_grid: &NavGrid,
    movement_input: &mut MovementInput,
    pursuit: &mut Pursuit,
) {
    movement_input.x_direction = 0.0;
    movement_input.z_direction = 0;

    let x_distance = target_position.x - mob_position.x;

    if mob_position.distance(target_position) > SENSE_DISTANCE {
        pursuit.path.clear();
        pursuit.seconds_until_repath = 0.0;
        return;
    }

    // Если уже достаточно подошли и слой совпадает, дальше не надо.
    if x_distance.abs() < TARGET_APPROACH_DISTANCE && mob_layer == target_layer {
        pursuit.path.clear();
        pursuit.seconds_until_repath = 0.0;
        return;
    }

    // Поправка, чтобы не подходить вплотную.
    target_position -= Vec2::new(x_distance.signum() * TARGET_APPROACH_DISTANCE, 0.0);

    pursuit.seconds_until_repath -= delta_secs;

    if pursuit.seconds_until_repath <= 0.0 || pursuit.path.is_empty() {
        pursuit.path = nav_grid
            .find_path(
                mob_position,
                mob_layer,
                target_position,
                target_layer,
                PATH_SEARCH_LIMIT,
            )
            .unwrap_or_default();
        pursuit.seconds_until_repath = REPATH_INTERVAL_SECS;
    }

    while pursuit.path.first().is_some_and(|segment| {
        segment.layer == mob_layer
            && (segment.position.x - mob_position.x).abs() <= WAYPOINT_REACH_DISTANCE
    }) {
        pursuit.path.remove(0);
    }

    let Some(waypoint) = pursuit.path.first() else {
        return;
    };

    let waypoint_x_distance = waypoint.position.x - mob_position.x;

    if waypoint_x_distance.abs() > WAYPOINT_REACH_DISTANCE {
        movement_input.x_direction = waypoint_x_distance.signum();
    } else {
        movement_input.z_direction = match mob_layer.cmp(&waypoint.layer) {
            Ordering::Equal => 0,
            Ordering::Greater => -1,
            Ordering::Less => 1,
        };
    }
}

////////////////////////////////////////////////////////////////////////////////

#[cfg(test)]
mod tests {
    use super::*;

    fn make_grid() -> NavGrid {
        let mut grid = NavGrid::test_grid(20, 3, 3);
        grid.test_fill_walkable();
        grid
    }

    fn path_segment(x: f32, layer: Layer) -> PathSegment {
        PathSegment {
            position: Vec2::new(x, 50.0),
            layer,
        }
    }

    #[test]
    fn mob_does_not_repath_when_player_outside_sense_distance() {
        let grid = make_grid();
        let mut movement_input = MovementInput::default();
        let mut pursuit = Pursuit {
            path: vec![path_segment(500.0, Layer::new(1))],
            seconds_until_repath: 0.1,
        };

        update_pursuit(
            0.1,
            Vec2::new(50.0, 50.0),
            Layer::new(1),
            Vec2::new(SENSE_DISTANCE + 500.0, 50.0),
            Layer::new(1),
            &grid,
            &mut movement_input,
            &mut pursuit,
        );

        assert_eq!(movement_input.x_direction, 0.0);
        assert_eq!(movement_input.z_direction, 0);
        assert!(pursuit.path.is_empty());
        assert_eq!(pursuit.seconds_until_repath, 0.0);
    }

    #[test]
    fn mob_repaths_when_player_inside_sense_distance() {
        let grid = make_grid();
        let mut movement_input = MovementInput::default();
        let mut pursuit = Pursuit::default();

        update_pursuit(
            0.1,
            Vec2::new(50.0, 50.0),
            Layer::new(1),
            Vec2::new(900.0, 50.0),
            Layer::new(1),
            &grid,
            &mut movement_input,
            &mut pursuit,
        );

        assert!(!pursuit.path.is_empty());
        assert_eq!(movement_input.x_direction, 1.0);
        assert_eq!(movement_input.z_direction, 0);
        assert_eq!(pursuit.seconds_until_repath, REPATH_INTERVAL_SECS);
    }

    #[test]
    fn mob_reuses_cached_path_before_repath_interval() {
        let grid = make_grid();
        let cached_waypoint = path_segment(650.0, Layer::new(1));
        let mut movement_input = MovementInput::default();
        let mut pursuit = Pursuit {
            path: vec![cached_waypoint.clone()],
            seconds_until_repath: REPATH_INTERVAL_SECS,
        };

        update_pursuit(
            0.1,
            Vec2::new(50.0, 50.0),
            Layer::new(1),
            Vec2::new(1_200.0, 50.0),
            Layer::new(1),
            &grid,
            &mut movement_input,
            &mut pursuit,
        );

        assert_eq!(pursuit.path.len(), 1);
        assert_eq!(pursuit.path[0].position, cached_waypoint.position);
        assert_eq!(pursuit.path[0].layer, cached_waypoint.layer);
        assert_eq!(movement_input.x_direction, 1.0);
        assert_eq!(movement_input.z_direction, 0);
    }

    #[test]
    fn mob_repaths_after_interval_for_moving_player() {
        let grid = make_grid();
        let cached_waypoint = path_segment(650.0, Layer::new(1));
        let mut movement_input = MovementInput::default();
        let mut pursuit = Pursuit {
            path: vec![cached_waypoint.clone()],
            seconds_until_repath: 0.0,
        };

        update_pursuit(
            REPATH_INTERVAL_SECS,
            Vec2::new(50.0, 50.0),
            Layer::new(1),
            Vec2::new(900.0, 50.0),
            Layer::new(1),
            &grid,
            &mut movement_input,
            &mut pursuit,
        );

        assert!(!pursuit.path.is_empty());
        assert_ne!(pursuit.path[0].position, cached_waypoint.position);
        assert_eq!(movement_input.x_direction, 1.0);
    }

    #[test]
    fn mob_retries_after_failed_path() {
        let mut grid = NavGrid::test_grid(20, 3, 1);
        let mut movement_input = MovementInput::default();
        let mut pursuit = Pursuit::default();

        update_pursuit(
            REPATH_INTERVAL_SECS,
            Vec2::new(50.0, 50.0),
            Layer::new(1),
            Vec2::new(900.0, 50.0),
            Layer::new(1),
            &grid,
            &mut movement_input,
            &mut pursuit,
        );

        assert!(pursuit.path.is_empty());
        assert_eq!(movement_input.x_direction, 0.0);

        grid.test_fill_walkable();

        update_pursuit(
            REPATH_INTERVAL_SECS,
            Vec2::new(50.0, 50.0),
            Layer::new(1),
            Vec2::new(900.0, 50.0),
            Layer::new(1),
            &grid,
            &mut movement_input,
            &mut pursuit,
        );

        assert!(!pursuit.path.is_empty());
        assert_eq!(movement_input.x_direction, 1.0);
    }

    #[test]
    fn mob_moves_between_layers_when_waypoint_requires_it() {
        let grid = make_grid();
        let mut movement_input = MovementInput::default();
        let mut pursuit = Pursuit::default();

        update_pursuit(
            REPATH_INTERVAL_SECS,
            Vec2::new(50.0, 50.0),
            Layer::new(1),
            Vec2::new(50.0, 50.0),
            Layer::new(2),
            &grid,
            &mut movement_input,
            &mut pursuit,
        );

        assert_eq!(movement_input.x_direction, 0.0);
        assert_eq!(movement_input.z_direction, 1);
    }
}

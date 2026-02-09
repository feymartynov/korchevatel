use std::{cmp::Ordering, collections::VecDeque};

use bevy::prelude::*;

use crate::{level::Layer, movement::MovementInput, player::Player};

use super::Mob;

/// Дистанция, на которую походить к цели, чтобы не толкаться вплотную.
const TARGET_APPROACH_DISTANCE: f32 = 400.0;

pub(super) fn plugin(app: &mut App) {
    app.add_systems(
        FixedUpdate,
        (
            setup_on_mob_added,
            setup_on_player_added,
            resolve_target,
            pop_command,
            execute_command,
        ),
    );
}

/// Очередь команд моба.
#[derive(Component, Debug, Default, Reflect)]
#[reflect(Component)]
struct CommandQueue {
    commands: VecDeque<MobCommand>,
}

impl CommandQueue {
    /// Добавляет команду в конец очереди.
    fn push(&mut self, cmd: MobCommand) {
        self.commands.push_back(cmd);
    }
}

/// Команда моба.
#[derive(Debug, Reflect)]
enum MobCommand {
    /// Движение.
    Move { distance: f32, layer: Layer },
}

/// Состояние выполняемой команды.
#[derive(Component, Debug)]
enum ActiveMobCommand {
    /// Движение.
    Move {
        start_x: f32,
        required_distance: f32,
        target_layer: Layer,
        last_x: Option<f32>,
        last_layer: Option<Layer>,
    },
}

#[derive(Component, Debug, Reflect)]
#[reflect(Component)]
struct Target(Entity);

/// Настройка при спавне моба.
fn setup_on_mob_added(
    mob_q: Query<Entity, (Added<Mob>, Without<Target>)>,
    player_q: Query<Entity, With<Player>>,
    mut commands: Commands,
) {
    let maybe_player = player_q.single().ok();

    for entity in mob_q {
        // Добавляем компоненты AI.
        commands.entity(entity).insert(CommandQueue::default());

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

/// Решает как достичь цели и задаёт команды.
fn resolve_target(
    mob_q: Query<(&Target, &GlobalTransform, &mut CommandQueue), Without<ActiveMobCommand>>,
    target_q: Query<(&GlobalTransform, &Layer)>,
) {
    for (target, mob_global_transform, mut command_queue) in mob_q {
        let Ok((target_global_transform, target_layer)) = target_q.get(target.0) else {
            continue;
        };

        let target_x = target_global_transform.translation().x;
        let mob_x = mob_global_transform.translation().x;
        let distance = target_x - mob_x;

        // Если уже достаточно подошли, дальше не надо.
        if distance.abs() < TARGET_APPROACH_DISTANCE {
            continue;
        }

        let approach_distance = distance - distance.signum() * TARGET_APPROACH_DISTANCE;

        command_queue.push(MobCommand::Move {
            distance: approach_distance,
            layer: *target_layer,
        });
    }
}

/// Берёт следующую команду из очереди на выполнение, если моб ничего не делает.
fn pop_command(
    mut commands: Commands,
    q: Query<(Entity, &GlobalTransform, &mut CommandQueue), Without<ActiveMobCommand>>,
) {
    for (entity, global_transform, mut queue) in q {
        let Some(cmd) = queue.commands.pop_front() else {
            continue;
        };

        match cmd {
            MobCommand::Move { distance, layer } => {
                commands.entity(entity).insert(ActiveMobCommand::Move {
                    start_x: global_transform.translation().x,
                    required_distance: distance,
                    target_layer: layer,
                    last_x: None,
                    last_layer: None,
                });
            }
        }
    }
}

/// Выполняет текущую команду.
fn execute_command(
    q: Query<(
        Entity,
        &GlobalTransform,
        &Layer,
        &mut MovementInput,
        &mut ActiveMobCommand,
    )>,
    mut commands: Commands,
) {
    for (entity, global_transform, layer, mut movement_input, mut active_command) in q {
        match active_command.as_mut() {
            ActiveMobCommand::Move {
                start_x,
                required_distance,
                target_layer,
                last_x,
                last_layer,
            } => {
                let current_x = global_transform.translation().x;

                // Если застрял, сбрасываем команду.
                if let Some(x) = last_x
                    && *x == current_x
                    && let Some(l) = last_layer
                    && l == layer
                {
                    warn!("Mob {entity} has stuck");
                    commands.entity(entity).remove::<ActiveMobCommand>();
                    continue;
                }

                *last_x = Some(current_x);
                *last_layer = Some(*layer);
                let traveled = current_x - *start_x;

                // Если надо ещё пройти по X, идём задаём движение в нужную сторону.
                movement_input.x_direction = if traveled.abs() < required_distance.abs() {
                    required_distance.signum()
                } else {
                    0.0
                };

                // Если нужно перейти на другой слой, пробуем это сделать.
                movement_input.z_direction = match layer.cmp(&target_layer) {
                    Ordering::Equal => 0,
                    Ordering::Greater => -1,
                    Ordering::Less => 1,
                };

                // Если уже не нужно никуда идти, ни по X, ни по Z, значит команда выполнена.
                if movement_input.x_direction == 0.0 && movement_input.z_direction == 0 {
                    commands.entity(entity).remove::<ActiveMobCommand>();
                }
            }
        }
    }
}

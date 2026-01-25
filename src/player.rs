use avian2d::prelude::*;
use bevy::prelude::*;

use crate::camera::FocusPoint;
use crate::character::{Attack, Registry as CharacterRegistry};
use crate::level::Layer;
use crate::movement::{MovementInput, MovementMessage};

pub(super) fn plugin(app: &mut App) {
    app.register_type::<Player>();
    app.register_type::<PlayerSpawnPoint>();
    app.add_systems(Update, (spawn, control));
}

/// Игровой персонаж
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq, Default, Reflect)]
#[require(Transform, Visibility)]
#[reflect(Component)]
pub struct Player;

/// Точка, где появляется игрок
#[derive(Component, Debug, Clone, PartialEq, Eq, Default, Reflect)]
#[require(Transform)]
#[reflect(Component)]
pub struct PlayerSpawnPoint {
    character: String,
}

// Создание игрового персонажа
fn spawn(
    player_spawn_point_q: Query<(&Transform, &Layer, &PlayerSpawnPoint), Added<Layer>>,
    player_q: Query<Entity, With<Player>>,
    mut camera_q: Query<Entity, With<IsDefaultUiCamera>>,
    mut commands: Commands,
) {
    let Ok((transform, layer, spawn_point)) = player_spawn_point_q.single() else {
        return;
    };

    if !player_q.is_empty() {
        error!("Player already spawned");
        return;
    }

    let Some(character) = CharacterRegistry::with(&spawn_point.character, |c| c.cloned()) else {
        error!("Missing player character {}", spawn_point.character);
        return;
    };

    let player = commands
        .spawn((
            Name::new("Player"),
            character.clone(),
            Player,
            *transform,
            *layer,
        ))
        .id();

    // Наводим камеру на игрока
    if let Ok(camera) = camera_q.single_mut() {
        commands.entity(camera).insert(FocusPoint::new(player));
    }
}

// Управление игровым персонажем
fn control(
    physics_time: Res<Time<Physics>>,
    mut movement_event_writer: MessageWriter<MovementMessage>,
    mut q: Query<(Entity, &mut MovementInput, &mut Attack), With<Player>>,
    keyboard_input: Res<ButtonInput<KeyCode>>,
) {
    if physics_time.is_paused() {
        return;
    }

    let Ok((player, mut movement_input, mut attack)) = q.single_mut() else {
        return;
    };

    // Перемещение
    let left = keyboard_input.pressed(KeyCode::KeyA);
    let right = keyboard_input.pressed(KeyCode::KeyD);
    movement_input.x_direction = (right as i8 - left as i8).into();

    let backward = keyboard_input.just_pressed(KeyCode::KeyW);
    let forward = keyboard_input.just_pressed(KeyCode::KeyS);
    let z_direction = forward as i8 - backward as i8;

    if z_direction != 0 {
        // Тут шлём через события, т.к. just_pressed не синхронизирован с FixedUpdate
        movement_event_writer.write(MovementMessage::Z {
            entity: player,
            z_direction,
        });
    }

    // Атака
    attack.set_attacking(keyboard_input.pressed(KeyCode::Space));
}

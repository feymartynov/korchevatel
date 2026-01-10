use bevy::prelude::*;

use crate::camera::FocusPoint;
use crate::character::Character;
use crate::level::Layer;
use crate::movement::MovementInput;

pub(super) fn plugin(app: &mut App) {
    app.register_type::<PlayerSpawnPoint>();
    app.add_systems(Update, (spawn, control));
}

/// Игровой персонаж
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq, Default)]
#[require(Transform, Visibility, Character)]
pub struct Player;

/// Точка, где появляется игрок
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq, Default, Reflect)]
#[require(Transform)]
#[reflect(Component)]
pub struct PlayerSpawnPoint;

// Создание игрового персонажа
fn spawn(
    player_spawn_point_q: Query<
        (&Transform, &Layer, &ChildOf),
        (With<PlayerSpawnPoint>, Added<Layer>),
    >,
    player_q: Query<Entity, With<Player>>,
    mut camera_q: Query<Entity, With<IsDefaultUiCamera>>,
    mut commands: Commands,
) {
    let Ok((transform, layer, child_of)) = player_spawn_point_q.single() else {
        return;
    };

    if !player_q.is_empty() {
        error!("Player already spawned");
        return;
    }

    let player = commands
        .spawn((
            Name::new("Player"),
            Character,
            Player,
            *transform,
            *layer,
            child_of.clone(),
        ))
        .id();

    // Наводим камеру на игрока
    if let Ok(camera) = camera_q.single_mut() {
        commands.entity(camera).insert(FocusPoint::new(player));
    }
}

// Управление игровым персонажем
fn control(
    mut movement_input_q: Query<&mut MovementInput, With<Player>>,
    keyboard_input: Res<ButtonInput<KeyCode>>,
) {
    let Ok(mut movement_input) = movement_input_q.single_mut() else {
        return;
    };

    let left = keyboard_input.pressed(KeyCode::KeyA);
    let right = keyboard_input.pressed(KeyCode::KeyD);
    movement_input.direction = (right as i8 - left as i8).into();
}

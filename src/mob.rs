mod ai;

use bevy::prelude::*;

use crate::character::Registry as CharacterRegistry;
use crate::level::Layer;

pub(super) fn plugin(app: &mut App) {
    app.register_type::<Mob>();
    app.register_type::<MobSpawnPoint>();
    app.add_systems(FixedUpdate, spawn);
    app.add_plugins(ai::plugin);
}

/// Неигровой персонаж
#[derive(Component, Default, Reflect)]
#[require(Transform, Visibility)]
#[reflect(Component)]
pub struct Mob;

/// Точка, где появляется моб
#[derive(Component, Default, Reflect)]
#[require(Transform)]
#[reflect(Component)]
pub struct MobSpawnPoint {
    character: String,
}

// Создание неигрового персонажа
fn spawn(q: Query<(&Transform, &Layer, &MobSpawnPoint), Added<Layer>>, mut commands: Commands) {
    for (transform, layer, spawn_point) in q {
        let Some(character) = CharacterRegistry::with(&spawn_point.character, |c| c.cloned())
        else {
            error!("Missing mob character {}", spawn_point.character);
            return;
        };

        commands.spawn((
            Name::new(character.name().to_string()),
            character,
            Mob,
            *transform,
            *layer,
        ));
    }
}

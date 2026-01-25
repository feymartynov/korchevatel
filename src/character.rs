mod animation;
mod attack;

use std::time::Duration;

use avian2d::prelude::*;
use bevy::prelude::*;
use bevy::sprite::Anchor;

use crate::movement::MovementBundle;

pub use self::animation::{Animation, AnimationState, AttackMode, MovementMode};
pub use self::attack::Attack;

////////////////////////////////////////////////////////////////////////////////

pub static CHARACTER_NIKITA: Character = Character {
    name: "Nikita",
    sprite_sheet: SpriteSheet {
        path: "images/characters/nikita.png",
        size: UVec2::new(227, 240),
        columns: 15,
        rows: 1,
    },
    anchor: Vec2::new(-0.12, 0.0),
    collider_radius: 40.0,
    collider_length: 150.0,
    animations: &[
        AnimationConfig {
            state: AnimationState {
                movement_mode: MovementMode::Idle,
                attack_mode: AttackMode::None,
            },
            duration: Duration::from_millis(500),
            sprite_indexes: &[0],
        },
        AnimationConfig {
            state: AnimationState {
                movement_mode: MovementMode::Walking,
                attack_mode: AttackMode::None,
            },
            duration: Duration::from_millis(100),
            sprite_indexes: &[1, 2, 3, 4, 5, 6],
        },
        AnimationConfig {
            state: AnimationState {
                movement_mode: MovementMode::Idle,
                attack_mode: AttackMode::Firing,
            },
            duration: Duration::from_millis(100),
            sprite_indexes: &[7, 8],
        },
        AnimationConfig {
            state: AnimationState {
                movement_mode: MovementMode::Walking,
                attack_mode: AttackMode::Firing,
            },
            duration: Duration::from_millis(100),
            sprite_indexes: &[9, 10, 11, 12, 13, 14],
        },
    ],
};

////////////////////////////////////////////////////////////////////////////////

pub(super) fn plugin(app: &mut App) {
    app.add_plugins(animation::plugin);
    app.add_observer(on_insert);
}

/// Персонаж
#[derive(Component, Clone)]
pub struct Character {
    name: &'static str,
    sprite_sheet: SpriteSheet,
    anchor: Vec2,
    collider_radius: f32,
    collider_length: f32,
    animations: &'static [AnimationConfig],
}

#[derive(Clone, Debug)]
struct SpriteSheet {
    path: &'static str,
    size: UVec2,
    columns: u32,
    rows: u32,
}

impl SpriteSheet {
    fn to_atlas_layout(&self) -> TextureAtlasLayout {
        TextureAtlasLayout::from_grid(self.size, self.columns, self.rows, None, None)
    }
}

#[derive(Clone, Debug)]
struct AnimationConfig {
    state: AnimationState,
    duration: Duration,
    sprite_indexes: &'static [usize],
}

/// Добавляет графику, физику и логику персонажа (играбельного или нет) к сущности
fn on_insert(
    inserted: On<Insert, Character>,
    q: Query<&Character>,
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut texture_atlas_layouts: ResMut<Assets<TextureAtlasLayout>>,
) {
    let Ok(character) = q.get(inserted.entity) else {
        error!("Missing Character");
        return;
    };

    let layout = character.sprite_sheet.to_atlas_layout();
    let texture_atlas_layout = texture_atlas_layouts.add(layout);
    let mut animation = Animation::default();

    for animation_config in character.animations {
        animation = animation.register_state(
            animation_config.state,
            animation_config.duration,
            animation_config.sprite_indexes.to_vec(),
        );
    }

    commands.entity(inserted.entity).insert((
        Name::new(character.name),
        Anchor::from(character.anchor),
        RigidBody::Dynamic,
        Collider::capsule(character.collider_radius, character.collider_length),
        LockedAxes::ROTATION_LOCKED,
        MovementBundle::default(),
        Attack::default(),
        Sprite {
            image: asset_server.load(character.sprite_sheet.path),
            texture_atlas: Some(TextureAtlas {
                layout: texture_atlas_layout,
                index: 0,
            }),
            ..Default::default()
        },
        animation,
    ));
}

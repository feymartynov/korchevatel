mod animation;
mod attack;
mod registry;

use std::time::Duration;

use avian2d::prelude::*;
use bevy::prelude::*;
use bevy::sprite::Anchor;

use crate::movement::MovementBundle;

use self::attack::Attack;

pub use self::animation::{Animation, AnimationState};
pub use self::attack::AttackMessage;
pub use self::registry::Registry;

pub(super) fn plugin(app: &mut App) {
    app.add_plugins((animation::plugin, attack::plugin));
    app.add_observer(on_insert);
}

/// Персонаж
#[derive(Component, Clone, Debug, Deserialize)]
pub struct Character {
    name: String,
    sprite_sheet: SpriteSheet,
    anchor: Vec2,
    collider_radius: f32,
    collider_length: f32,
    animations: Vec<AnimationConfig>,
}

impl Character {
    pub fn name(&self) -> &str {
        &self.name
    }
}

#[derive(Clone, Debug, Deserialize)]
struct SpriteSheet {
    path: String,
    size: UVec2,
    columns: u32,
    rows: u32,
}

impl SpriteSheet {
    fn to_atlas_layout(&self) -> TextureAtlasLayout {
        TextureAtlasLayout::from_grid(self.size, self.columns, self.rows, None, None)
    }
}

#[derive(Clone, Debug, Deserialize)]
struct AnimationConfig {
    #[serde(flatten)]
    state: AnimationState,
    #[serde(with = "humantime_serde")]
    duration: Duration,
    sprite_indexes: Vec<usize>,
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

    for animation_config in &character.animations {
        animation = animation.register_state(
            animation_config.state,
            animation_config.duration,
            animation_config.sprite_indexes.to_vec(),
        );
    }

    commands.entity(inserted.entity).insert((
        Anchor::from(character.anchor),
        RigidBody::Dynamic,
        Collider::capsule(character.collider_radius, character.collider_length),
        LockedAxes::ROTATION_LOCKED,
        MovementBundle::default(),
        Attack::default(),
        Sprite {
            image: asset_server.load(&character.sprite_sheet.path),
            texture_atlas: Some(TextureAtlas {
                layout: texture_atlas_layout,
                index: 0,
            }),
            ..Default::default()
        },
        animation,
    ));
}

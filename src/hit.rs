use avian2d::prelude::*;
use bevy::prelude::*;

use crate::level::Layer;

pub(super) fn plugin(app: &mut App) {
    app.add_message::<HitMessage>();
}

#[derive(Message, Clone, Debug)]
pub struct HitMessage {
    pub hit_data: ShapeHitData,
    pub global_transform: GlobalTransform,
    pub layer: Layer,
    pub origin: Vec2,
    pub hit_point: Vec2,
    pub is_character: bool,
}

impl HitMessage {
    pub fn entity(&self) -> Entity {
        self.hit_data.entity
    }
}
